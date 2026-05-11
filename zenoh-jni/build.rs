use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::quote;

use zenoh_flat::core::{
    FunctionsConverter, InputFn, NO_INPUT, NO_OUTPUT, NameMangler, OutputFn, TypeRegistry,
    TypesConverter, input_result, output_result, primitive_builtins, result_wire_type,
};
use zenoh_flat::jni::{CallbacksConverter, JniDecoderStruct, JniTryClosureBody};
use zenoh_flat::kotlin::{KotlinInterfaceGenerator, KotlinTypeMap};

// =====================================================================
// Helpers — private constructors returning `InputFn` / `OutputFn`.
// One closure shape each; replace the previous local `macro_rules!`.
//
// Helpers take `&str` because `InputFn::new` / `OutputFn::new` require
// `Fn + Send + Sync + 'static`, and `syn::Path` / `syn::Type` /
// `syn::Expr` are not `Sync` (they may carry `proc_macro2::TokenStream`).
// We capture an owned `String` and parse inside the closure — matches
// the pattern already used by `prebindgen-ext::jni::inline_fn_helpers`.
// =====================================================================

fn decode_pure(path: &str) -> InputFn {
    let s = path.to_string();
    InputFn::new(move |input: &syn::Ident| -> TokenStream {
        let p: syn::Path = syn::parse_str(&s).expect("decode_pure: invalid path");
        quote! { #p(#input)? }
    })
}

fn decode_env_ref_mut(path: &str) -> InputFn {
    let s = path.to_string();
    InputFn::new(move |input: &syn::Ident| -> TokenStream {
        let p: syn::Path = syn::parse_str(&s).expect("decode_env_ref_mut: invalid path");
        quote! { #p(&mut env, &#input)? }
    })
}

fn decode_option_env_ref_mut(path: &str) -> InputFn {
    let s = path.to_string();
    InputFn::new(move |input: &syn::Ident| -> TokenStream {
        let p: syn::Path = syn::parse_str(&s).expect("decode_option_env_ref_mut: invalid path");
        quote! {
            if !#input.is_null() {
                Some(#p(&mut env, &#input)?)
            } else {
                None
            }
        }
    })
}

fn decode_owned_raw(owned_object: &str) -> InputFn {
    let s = owned_object.to_string();
    InputFn::new(move |input: &syn::Ident| -> TokenStream {
        let p: syn::Path = syn::parse_str(&s).expect("decode_owned_raw: invalid path");
        quote! { #p::from_raw(#input) }
    })
}

/// Reconstruct an `Arc<T>` from a raw pointer, clone the inner `T`, and
/// let the temporary `Arc` drop at end of scope.
fn decode_arc_from_raw() -> InputFn {
    InputFn::new(|input: &syn::Ident| -> TokenStream {
        quote! { (*std::sync::Arc::from_raw(#input)).clone() }
    })
}

fn decode_option_arc_from_raw(inner: &str) -> InputFn {
    let s = inner.to_string();
    InputFn::new(move |input: &syn::Ident| -> TokenStream {
        let inner: syn::Type =
            syn::parse_str(&s).expect("decode_option_arc_from_raw: invalid inner type");
        quote! {
            if #input != 0 {
                Some(unsafe {
                    let raw = #input as *const #inner;
                    (*raw).clone()
                })
            } else {
                None
            }
        }
    })
}

fn encode_wrapper(path: &str) -> OutputFn {
    let s = path.to_string();
    OutputFn::new(move |output: Option<&syn::Ident>| -> TokenStream {
        let p: syn::Path = syn::parse_str(&s).expect("encode_wrapper: invalid path");
        match output {
            Some(o) => quote! { #p(&mut env, #o)? },
            None => quote! { std::ptr::null_mut() },
        }
    })
}

fn encode_arc_into_raw() -> OutputFn {
    OutputFn::new(|output: Option<&syn::Ident>| -> TokenStream {
        match output {
            Some(o) => quote! { std::sync::Arc::into_raw(std::sync::Arc::new(#o)) },
            None => quote! { std::ptr::null() },
        }
    })
}

/// `Some(v)` → `Arc::into_raw(Arc::new(v.clone())) as i64`; `None` → `0`.
fn encode_option_clone_into_arc_raw_jlong() -> OutputFn {
    OutputFn::new(|output: Option<&syn::Ident>| -> TokenStream {
        match output {
            Some(o) => quote! {
                #o.as_ref()
                    .map(|value| std::sync::Arc::into_raw(std::sync::Arc::new(value.clone())) as i64)
                    .unwrap_or(0)
            },
            None => quote! { 0 },
        }
    })
}

/// `<value> as <wire>` on success, `<on_err> as <wire>` on the throw path.
fn encode_cast(wire: &str, on_err: &str) -> OutputFn {
    let wire = wire.to_string();
    let on_err = on_err.to_string();
    OutputFn::new(move |output: Option<&syn::Ident>| -> TokenStream {
        let wire: syn::Type = syn::parse_str(&wire).expect("encode_cast: invalid wire type");
        let on_err: syn::Expr =
            syn::parse_str(&on_err).expect("encode_cast: invalid on_err expr");
        match output {
            Some(o) => quote! { #o as #wire },
            None => quote! { #on_err as #wire },
        }
    })
}

// =====================================================================
// Bindings — split into two passes for visual clarity.
//   * primitive_bindings — universal primitives + ZResult<_> wildcard
//   * legacy_bindings    — hand-written zenoh-jni decoders/encoders
//                          and zenoh-specific opaque-handle types.
// =====================================================================

fn primitive_bindings() -> TypeRegistry {
    primitive_builtins()
        // ZResult<T> wildcard. The body strategy's `?` already unwraps the
        // result, so the inner encoder receives the unwrapped T directly.
        .wrap_type_wire("ZResult<_>", input_result, output_result, result_wire_type)
        // Unit return — no encoder/decoder.
        .type_pair("ZResult<()>", "()", NO_INPUT, NO_OUTPUT)
        // ZResult<bool>: explicit row because bool_output uses the
        // struct-encoder reference convention. The wildcard would
        // synthesize the wrong shape for return values.
        .type_pair(
            "ZResult<bool>",
            "jni::sys::jboolean",
            NO_INPUT,
            encode_cast("jni::sys::jboolean", "false"),
        )
}

/// Hand-written decoders/encoders that point at symbols in the zenoh-jni
/// source tree (`crate::utils::*`, `crate::sample_callback::*`,
/// `crate::zenoh_id::*`, `crate::owned_object::*`) plus opaque-handle
/// types declared by `zenoh` itself. Each row is a candidate for removal
/// once the matching auto-generation strategy lands in `prebindgen-ext`
/// (see the long-term plan).
fn legacy_bindings() -> TypeRegistry {
    TypeRegistry::new()
        // Java-enum-shaped types (decode `jint` → enum). Future:
        // auto-generate from `#[prebindgen]` enums in zenoh-flat.
        .type_pair(
            "CongestionControl",
            "jni::sys::jint",
            decode_pure("crate::utils::decode_congestion_control"),
            NO_OUTPUT,
        )
        .type_pair(
            "Priority",
            "jni::sys::jint",
            decode_pure("crate::utils::decode_priority"),
            NO_OUTPUT,
        )
        .type_pair(
            "Reliability",
            "jni::sys::jint",
            decode_pure("crate::utils::decode_reliability"),
            NO_OUTPUT,
        )
        .type_pair(
            "QueryTarget",
            "jni::sys::jint",
            decode_pure("crate::utils::decode_query_target"),
            NO_OUTPUT,
        )
        .type_pair(
            "ConsolidationMode",
            "jni::sys::jint",
            decode_pure("crate::utils::decode_consolidation"),
            NO_OUTPUT,
        )
        .type_pair(
            "ReplyKeyExpr",
            "jni::sys::jint",
            decode_pure("crate::utils::decode_reply_key_expr"),
            NO_OUTPUT,
        )
        // Manual callback signatures. CallbacksConverter skips signatures
        // already in the type registry, which lets these stay opt-out.
        // Future: drop them and let CallbacksConverter own the symbol shape.
        .type_pair(
            "impl Fn(Query) + Send + Sync + 'static",
            "jni::objects::JObject",
            decode_env_ref_mut("crate::sample_callback::process_kotlin_query_callback"),
            NO_OUTPUT,
        )
        .type_pair(
            "impl Fn(Reply) + Send + Sync + 'static",
            "jni::objects::JObject",
            decode_env_ref_mut("crate::sample_callback::process_kotlin_reply_callback"),
            NO_OUTPUT,
        )
        .type_pair(
            "impl Fn() + Send + Sync + 'static",
            "jni::objects::JObject",
            decode_env_ref_mut("crate::sample_callback::process_kotlin_on_close_callback"),
            NO_OUTPUT,
        )
        // KeyExpr `ptr: Long` field round-trip. Future: a generic
        // Arc-handle field strategy in JniDecoderStruct.
        .type_pair(
            "Option<ZKeyExpr<'static>>",
            "jni::sys::jlong",
            decode_option_arc_from_raw("zenoh::key_expr::KeyExpr<'static>"),
            encode_option_clone_into_arc_raw_jlong(),
        )
        // KeyExpr borrows / returns. The by-value `KeyExpr` row is
        // auto-registered by JniDecoderStruct from keyexpr.rs's struct,
        // so only borrow + result variants are manual.
        .type_pair(
            "&KeyExpr",
            "jni::objects::JObject",
            decode_env_ref_mut("decode_KeyExpr"),
            NO_OUTPUT,
        )
        .type_pair(
            "ZResult<KeyExpr>",
            "jni::sys::jobject",
            NO_INPUT,
            encode_wrapper("encode_KeyExpr"),
        )
        // `relation_to`'s SetIntersectionLevel — zenoh enum cast to jint.
        .type_pair(
            "ZResult<SetIntersectionLevel>",
            "jni::sys::jint",
            NO_INPUT,
            encode_cast("jni::sys::jint", "-1"),
        )
        // Encoding via JObject + custom decoder. Future: declare
        // Encoding as a flat #[prebindgen] struct in zenoh-flat.
        .type_pair(
            "Encoding",
            "jni::objects::JObject",
            decode_env_ref_mut("crate::utils::decode_jni_encoding"),
            NO_OUTPUT,
        )
        .type_pair(
            "Option<Encoding>",
            "jni::objects::JObject",
            decode_option_env_ref_mut("crate::utils::decode_jni_encoding"),
            NO_OUTPUT,
        )
        // Opaque borrows: `Arc<T>` handles received as `*const T`. Future:
        // prebindgen-ext::jni::opaque::OwnedObject.
        .type_pair(
            "&Session",
            "*const Session",
            decode_owned_raw("crate::owned_object::OwnedObject"),
            NO_OUTPUT,
        )
        .type_pair(
            "&Config",
            "*const Config",
            decode_owned_raw("crate::owned_object::OwnedObject"),
            NO_OUTPUT,
        )
        // `drop_session(session: Session)` consumes the Arc.
        .type_pair(
            "Session",
            "*const Session",
            decode_arc_from_raw(),
            NO_OUTPUT,
        )
        // ZenohId encoders. Future: byte-array struct strategy + a
        // generic `Vec<T>` → `List<wire-of-T>` wildcard.
        .type_pair(
            "ZResult<ZenohId>",
            "jni::sys::jbyteArray",
            NO_INPUT,
            encode_wrapper("crate::zenoh_id::zenoh_id_to_byte_array"),
        )
        .type_pair(
            "ZResult<Vec<ZenohId>>",
            "jni::sys::jobject",
            NO_INPUT,
            encode_wrapper("crate::zenoh_id::zenoh_ids_to_java_list"),
        )
        // Opaque returns: `Arc::into_raw` for every handle shape. Future:
        // a generic Arc<T> return wildcard pattern.
        .type_pair(
            "ZResult<Session>",
            "*const Session",
            NO_INPUT,
            encode_arc_into_raw(),
        )
        .type_pair(
            "ZResult<Publisher<'static>>",
            "*const Publisher<'static>",
            NO_INPUT,
            encode_arc_into_raw(),
        )
        .type_pair(
            "ZResult<Subscriber<()>>",
            "*const Subscriber<()>",
            NO_INPUT,
            encode_arc_into_raw(),
        )
        .type_pair(
            "ZResult<Querier<'static>>",
            "*const Querier<'static>",
            NO_INPUT,
            encode_arc_into_raw(),
        )
        .type_pair(
            "ZResult<Queryable<()>>",
            "*const Queryable<()>",
            NO_INPUT,
            encode_arc_into_raw(),
        )
        .type_pair(
            "ZResult<AdvancedSubscriber<()>>",
            "*const AdvancedSubscriber<()>",
            NO_INPUT,
            encode_arc_into_raw(),
        )
        .type_pair(
            "ZResult<AdvancedPublisher<'static>>",
            "*const AdvancedPublisher<'static>",
            NO_INPUT,
            encode_arc_into_raw(),
        )
}

/// Rust → Kotlin name map consumed by `KotlinInterfaceGenerator`.
fn shared_kotlin_types() -> KotlinTypeMap {
    KotlinTypeMap::new()
        .with_primitive_builtins()
        .add("String", "String")
        .add("Option<String>", "String")
        .add("Vec<u8>", "ByteArray")
        .add("Option<Vec<u8>>", "ByteArray")
        // `impl Fn(Sample) + …` Kotlin FQN is registered automatically
        // by `CallbacksConverter`.
        .add(
            "impl Fn(Query) + Send + Sync + 'static",
            "io.zenoh.jni.callbacks.JNIQueryableCallback",
        )
        .add(
            "impl Fn(Reply) + Send + Sync + 'static",
            "io.zenoh.jni.callbacks.JNIGetCallback",
        )
        .add(
            "impl Fn() + Send + Sync + 'static",
            "io.zenoh.jni.callbacks.JNIOnCloseCallback",
        )
        .add("CongestionControl", "Int")
        .add("Priority", "Int")
        .add("Reliability", "Int")
        .add("QueryTarget", "Int")
        .add("ConsolidationMode", "Int")
        .add("ReplyKeyExpr", "Int")
        .add("Option<ZKeyExpr<'static>>", "Long")
        .add("&KeyExpr", "KeyExpr")
        .add("ZResult<KeyExpr>", "KeyExpr")
        .add("ZResult<SetIntersectionLevel>", "Int")
        .add("Encoding", "io.zenoh.jni.JNIEncoding")
        .add("Option<Encoding>", "io.zenoh.jni.JNIEncoding")
        .add("&Session", "Long")
        .add("&Config", "Long")
        .add("Session", "Long")
        .add("ZResult<ZenohId>", "ByteArray")
        .add("ZResult<Vec<ZenohId>>", "List<ByteArray>")
        .add("ZResult<Session>", "Long")
        .add("ZResult<Publisher<'static>>", "Long")
        .add("ZResult<Subscriber<()>>", "Long")
        .add("ZResult<Querier<'static>>", "Long")
        .add("ZResult<Queryable<()>>", "Long")
        .add("ZResult<AdvancedSubscriber<()>>", "Long")
        .add("ZResult<AdvancedPublisher<'static>>", "Long")
        .add("ZResult<bool>", "Boolean")
}

// =====================================================================
// main — single flat pipeline. Every #[prebindgen] item lands in one
// `JNINative` Kotlin class and one Rust bindings file. zenoh-flat's
// `pub use` re-exports make `source_module = "zenoh_flat"` resolve
// regardless of the declaring sub-module, so build.rs needs no
// awareness of where each item lives.
// =====================================================================

fn main() {
    let source = prebindgen::Source::new(zenoh_flat::PREBINDGEN_OUT_DIR);
    let registry = primitive_bindings().merge(legacy_bindings());

    // (1) Struct pass — JniDecoderStruct emits decode_/encode_ fns for
    //     each #[prebindgen] struct and registers the binding.
    let mut struct_conv = TypesConverter::builder(
        JniDecoderStruct::new("zenoh_flat", "crate::errors::ZResult")
            .java_class_prefix("io/zenoh/jni"),
    )
    .type_registry(registry)
    .build();
    let struct_items: Vec<_> = source
        .items_all()
        .batching(struct_conv.as_closure())
        .collect();
    let registry = struct_conv.into_type_registry();

    // (2) Callback pass — scans every fn signature for `impl Fn(...)`
    //     parameter types not already in the registry, emits
    //     `process_kotlin_<Stem>_callback` Rust fns, writes Kotlin
    //     fun-interface files. The stem is derived from the callback's
    //     parameter types (e.g. `impl Fn(Sample)` → `JNISampleCallback`).
    let mut cb_conv = CallbacksConverter::builder()
        .kotlin_package("io.zenoh.jni.callbacks")
        .kotlin_output_dir("../zenoh-jni-runtime/src/commonMain/kotlin/io/zenoh/jni/callbacks")
        .type_registry(registry)
        .kotlin_types(shared_kotlin_types().add("Sample", "io.zenoh.jni.Sample"))
        .build();
    let cb_items: Vec<_> = source.items_all().batching(cb_conv.as_closure()).collect();
    let cb_kotlin_types = cb_conv.kotlin_types().clone();
    let registry = cb_conv.into_type_registry();

    // (3) Function pass — single FunctionsConverter for the whole crate.
    let extra_leading: TokenStream = quote! {
        mut env: jni::JNIEnv,
        _class: jni::objects::JClass
    };
    let extra_attrs: Vec<syn::Attribute> = vec![
        syn::parse_quote!(#[no_mangle]),
        syn::parse_quote!(#[allow(non_snake_case, unused_mut, unused_variables)]),
    ];
    let mut fn_conv = FunctionsConverter::builder(JniTryClosureBody::new("crate::throw_exception"))
        .source_module("zenoh_flat")
        .name_mangler(NameMangler::CamelPrefixSuffix {
            prefix: "Java_io_zenoh_jni_JNINative_".into(),
            suffix: "ViaJNI".into(),
        })
        .extra_leading_params(extra_leading)
        .extra_attrs(extra_attrs)
        .extern_abi(syn::parse_quote!(extern "C"))
        .unsafety(true)
        .type_registry(registry.clone())
        .build();
    let fn_items: Vec<_> = source
        .items_all()
        .filter(|(item, _)| matches!(item, syn::Item::Fn(_)))
        .batching(fn_conv.as_closure())
        .collect();

    // (4) Pass-through: items that are neither structs nor fns
    //     (e.g. the prebindgen feature-mismatch assertion).
    let passthrough = source
        .items_all()
        .filter(|(item, _)| !matches!(item, syn::Item::Fn(_) | syn::Item::Struct(_)));

    let bindings_file = struct_items
        .into_iter()
        .chain(cb_items)
        .chain(fn_items)
        .chain(passthrough)
        .collect::<prebindgen::collect::Destination>()
        .write("zenoh_flat_jni.rs");
    println!(
        "cargo:warning=Generated bindings at: {}",
        bindings_file.display()
    );

    // (5) Kotlin interface — one class fed every item.
    let struct_names = [
        "HistoryConfig",
        "RecoveryConfig",
        "CacheConfig",
        "MissDetectionConfig",
        "KeyExpr",
        "Sample",
    ];
    let mut kotlin_types = shared_kotlin_types();
    for s in &struct_names {
        kotlin_types = kotlin_types.add(*s, *s).add(format!("Option<{}>", s), *s);
    }
    for (k, v) in cb_kotlin_types.iter() {
        kotlin_types = kotlin_types.add(k.as_str(), v.as_str());
    }

    let mut kotlin = KotlinInterfaceGenerator::builder()
        .output_path("../zenoh-jni/generated-kotlin/io/zenoh/jni/JNINative.kt")
        .package("io.zenoh.jni")
        .class_name("JNINative")
        .throws_class("io.zenoh.exceptions.ZError")
        .init_load("io.zenoh.ZenohLoad")
        .function_suffix("ViaJNI")
        .type_registry(registry)
        .kotlin_types(kotlin_types)
        .build();
    for (item, loc) in source.items_all() {
        kotlin.add_item(&item, &loc);
    }
    kotlin
        .write()
        .expect("failed to write generated JNINative.kt");
}
