use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::quote;

use zenoh_flat::core::{
    primitive_builtins, FunctionsConverter, InputFn, NameMangler, OutputFn, TypeRegistry,
    TypesConverter,
};
use zenoh_flat::jni::{JniDecoderStruct, JniTryClosureBody};
use zenoh_flat::kotlin::{KotlinInterfaceGenerator, KotlinTypeMap};

macro_rules! decode_pure {
    ($path:path) => {
        InputFn::new(|input: &syn::Ident| -> TokenStream {
            quote! { $path(#input)? }
        })
    };
}

macro_rules! decode_env_ref {
    ($path:path) => {
        InputFn::new(|input: &syn::Ident| -> TokenStream {
            quote! { $path(&env, &#input)? }
        })
    };
}

macro_rules! decode_env_ref_mut {
    ($path:path) => {
        InputFn::new(|input: &syn::Ident| -> TokenStream {
            quote! { $path(&mut env, &#input)? }
        })
    };
}

macro_rules! decode_option_env_ref {
    ($path:path) => {
        InputFn::new(|input: &syn::Ident| -> TokenStream {
            quote! {
                if !#input.is_null() {
                    Some($path(&env, &#input)?)
                } else {
                    None
                }
            }
        })
    };
}

macro_rules! decode_option_env_ref_mut {
    ($path:path) => {
        InputFn::new(|input: &syn::Ident| -> TokenStream {
            quote! {
                if !#input.is_null() {
                    Some($path(&mut env, &#input)?)
                } else {
                    None
                }
            }
        })
    };
}

macro_rules! decode_owned_raw {
    ($owned_object:path) => {
        InputFn::new(|input: &syn::Ident| -> TokenStream {
            quote! { $owned_object::from_raw(#input) }
        })
    };
}

/// Reconstruct an `Arc<T>` from a raw pointer, clone the inner `T`, and let
/// the temporary `Arc` drop at end of scope (releasing the JNI strong
/// reference).
macro_rules! decode_arc_from_raw {
    () => {
        InputFn::new(|input: &syn::Ident| -> TokenStream {
            quote! { (*std::sync::Arc::from_raw(#input)).clone() }
        })
    };
}

macro_rules! decode_option_arc_from_raw {
    ($inner:ty) => {
        InputFn::new(|input: &syn::Ident| -> TokenStream {
            quote! {
                if #input != 0 {
                    Some(unsafe {
                        let raw = #input as *const $inner;
                        (*raw).clone()
                    })
                } else {
                    None
                }
            }
        })
    };
}

macro_rules! encode_wrapper {
    ($path:path) => {
        OutputFn::new(|output: Option<&syn::Ident>| -> TokenStream {
            match output {
                Some(output) => quote! { $path(&mut env, #output)? },
                None => quote! { std::ptr::null_mut() },
            }
        })
    };
}

macro_rules! encode_arc_into_raw {
    () => {
        OutputFn::new(|output: Option<&syn::Ident>| -> TokenStream {
            match output {
                Some(output) => {
                    quote! { std::sync::Arc::into_raw(std::sync::Arc::new(#output)) }
                }
                None => quote! { std::ptr::null() },
            }
        })
    };
}

/// Encode an `Option<T>` (where `T: Clone`) into a `jlong` Arc-handle.
///
/// `Some(v)` becomes `Arc::into_raw(Arc::new(v.clone())) as i64`.
/// `None` maps to `0`.
macro_rules! encode_option_clone_into_arc_raw_jlong {
    () => {
        OutputFn::new(|output: Option<&syn::Ident>| -> TokenStream {
            match output {
                Some(output) => quote! {
                    #output
                        .as_ref()
                        .map(|value| std::sync::Arc::into_raw(std::sync::Arc::new(value.clone())) as i64)
                        .unwrap_or(0)
                },
                None => quote! { 0 },
            }
        })
    };
}

/// Emit `<result> as <wire>` on success and `<on_err> as <wire>` on the
/// throw-path. Used for primitive wire types that map straight from the
/// Rust return value (e.g. `bool` → `jboolean`, `i32` → `jint`).
macro_rules! encode_cast {
    ($wire:path, $on_err:expr) => {
        OutputFn::new(|output: Option<&syn::Ident>| -> TokenStream {
            match output {
                Some(output) => quote! { #output as $wire },
                None => quote! { $on_err as $wire },
            }
        })
    };
}

/// Wire-side `TypeRegistry` shared across every JNI surface
/// generated in this crate. Defined once, threaded into the struct-phase
/// converter, then forwarded — together with the auto-registered struct
/// bindings — into the methods phase and the Kotlin generator.
fn shared_bindings() -> TypeRegistry {
    primitive_builtins()
        // Strings & byte arrays.
        .type_pair("String", "jni::objects::JString")
        .input(decode_env_ref_mut!(crate::utils::decode_string))
        .output(OutputFn::new(|output: Option<&syn::Ident>| -> TokenStream {
            match output {
                Some(output) => quote! { env.new_string(#output).map_err(|err| zerror!(err))? },
                None => quote! { jni::objects::JString::from(jni::objects::JObject::null()) },
            }
        }))
        .type_pair("Option<String>", "jni::objects::JString")
        .input(decode_option_env_ref_mut!(crate::utils::decode_string))
        .type_pair("Vec<u8>", "jni::objects::JByteArray")
        .input(decode_env_ref!(crate::utils::decode_byte_array))
        .type_pair("Option<Vec<u8>>", "jni::objects::JByteArray")
        .input(decode_option_env_ref!(crate::utils::decode_byte_array))
        // Callbacks.
        .type_pair("impl Fn(Sample) + Send + Sync + 'static", "jni::objects::JObject")
        .input(decode_env_ref_mut!(crate::sample_callback::process_kotlin_sample_callback))
        .type_pair("impl Fn(Query) + Send + Sync + 'static", "jni::objects::JObject")
        .input(decode_env_ref_mut!(crate::sample_callback::process_kotlin_query_callback))
        .type_pair("impl Fn(Reply) + Send + Sync + 'static", "jni::objects::JObject")
        .input(decode_env_ref_mut!(crate::sample_callback::process_kotlin_reply_callback))
        .type_pair("impl Fn() + Send + Sync + 'static", "jni::objects::JObject")
        .input(decode_env_ref_mut!(crate::sample_callback::process_kotlin_on_close_callback))
        // Java-enum-shaped types.
        .type_pair("CongestionControl", "jni::sys::jint")
        .input(decode_pure!(crate::utils::decode_congestion_control))
        .type_pair("Priority", "jni::sys::jint")
        .input(decode_pure!(crate::utils::decode_priority))
        .type_pair("Reliability", "jni::sys::jint")
        .input(decode_pure!(crate::utils::decode_reliability))
        .type_pair("QueryTarget", "jni::sys::jint")
        .input(decode_pure!(crate::utils::decode_query_target))
        .type_pair("ConsolidationMode", "jni::sys::jint")
        .input(decode_pure!(crate::utils::decode_consolidation))
        .type_pair("ReplyKeyExpr", "jni::sys::jint")
        .input(decode_pure!(crate::utils::decode_reply_key_expr))
        // FlatKeyExpr (zenoh_flat::keyexpr::KeyExpr) — auto-generated as a
        // Kotlin data class; `ptr: Long` carries the raw Arc pointer (0 =
        // string-only). The flat Rust struct now stores
        // `Option<ZKeyExpr<'static>>`, so decode the primitive field by
        // temporarily materializing an Arc-handle (from raw), cloning the
        // inner key expression, then dropping the temporary Arc.
        //
        // `"KeyExpr"` (by-value) is auto-registered by the struct_conv pass
        // below, so only the borrow and return variants need manual entries.
        .type_pair("Option<ZKeyExpr<'static>>", "jni::sys::jlong")
        .input(decode_option_arc_from_raw!(zenoh::key_expr::KeyExpr<'static>))
        .output(encode_option_clone_into_arc_raw_jlong!())
        .type_pair("&KeyExpr", "jni::objects::JObject")
        .input(decode_env_ref_mut!(decode_KeyExpr))
        .type_pair("ZResult<KeyExpr>", "jni::sys::jobject")
        .output(encode_wrapper!(encode_KeyExpr))
        // Set-intersection level returned by `relation_to`. Cast zenoh's
        // enum (variants 0/1/2/3 in declaration order) directly to `jint`.
        .type_pair("ZResult<SetIntersectionLevel>", "jni::sys::jint")
        .output(encode_cast!(jni::sys::jint, -1))
        // Encoding via JObject + custom decoder.
        .type_pair("Encoding", "jni::objects::JObject")
        .input(decode_env_ref_mut!(crate::utils::decode_jni_encoding))
        .type_pair("Option<Encoding>", "jni::objects::JObject")
        .input(decode_option_env_ref_mut!(crate::utils::decode_jni_encoding))
        // Borrows: opaque Arc handles received as `*const T`.
        .type_pair("&Session", "*const Session")
        .input(decode_owned_raw!(crate::owned_object::OwnedObject))
        .type_pair("&Config", "*const Config")
        .input(decode_owned_raw!(crate::owned_object::OwnedObject))
        // Owning take by value: the wire side reconstructs the `Arc<Session>`
        // (releasing its strong reference at end of scope) and hands a cloned
        // `Session` to the wrapped fn. Used by `drop_session`.
        .type_pair("Session", "*const Session")
        .input(decode_arc_from_raw!())
        // Returns: ZenohId / Vec<ZenohId> via custom encoders.
        .type_pair("ZResult<ZenohId>", "jni::sys::jbyteArray")
        .output(encode_wrapper!(crate::zenoh_id::zenoh_id_to_byte_array))
        .type_pair("ZResult<Vec<ZenohId>>", "jni::sys::jobject")
        .output(encode_wrapper!(crate::zenoh_id::zenoh_ids_to_java_list))
        // Returns: opaque Arc handles.
        .type_pair("ZResult<Session>", "*const Session")
        .output(encode_arc_into_raw!())
        .type_pair("ZResult<Publisher<'static>>", "*const Publisher<'static>")
        .output(encode_arc_into_raw!())
        .type_pair("ZResult<Subscriber<()>>", "*const Subscriber<()>")
        .output(encode_arc_into_raw!())
        .type_pair("ZResult<Querier<'static>>", "*const Querier<'static>")
        .output(encode_arc_into_raw!())
        .type_pair("ZResult<Queryable<()>>", "*const Queryable<()>")
        .output(encode_arc_into_raw!())
        .type_pair("ZResult<AdvancedSubscriber<()>>", "*const AdvancedSubscriber<()>")
        .output(encode_arc_into_raw!())
        .type_pair("ZResult<AdvancedPublisher<'static>>", "*const AdvancedPublisher<'static>")
        .output(encode_arc_into_raw!())
        // Returns: bool primitive (wire matches Java's `boolean`).
        .type_pair("ZResult<bool>", "jni::sys::jboolean")
        .output(encode_cast!(jni::sys::jboolean, false))
        // Unit returns: ZResult<()> with `()` wire type so the converter treats it as a no-return shape.
        .type_pair("ZResult<()>", "()")
        // Structs from ext.rs and nullable wrappers.
        .type_pair("HistoryConfig", "jni::objects::JObject")
        .input(decode_env_ref_mut!(decode_HistoryConfig))
        .type_pair("Option<HistoryConfig>", "jni::objects::JObject")
        .input(decode_option_env_ref_mut!(decode_HistoryConfig))
        .type_pair("RecoveryConfig", "jni::objects::JObject")
        .input(decode_env_ref_mut!(decode_RecoveryConfig))
        .type_pair("Option<RecoveryConfig>", "jni::objects::JObject")
        .input(decode_option_env_ref_mut!(decode_RecoveryConfig))
        .type_pair("CacheConfig", "jni::objects::JObject")
        .input(decode_env_ref_mut!(decode_CacheConfig))
        .type_pair("Option<CacheConfig>", "jni::objects::JObject")
        .input(decode_option_env_ref_mut!(decode_CacheConfig))
        .type_pair("MissDetectionConfig", "jni::objects::JObject")
        .input(decode_env_ref_mut!(decode_MissDetectionConfig))
        .type_pair("Option<MissDetectionConfig>", "jni::objects::JObject")
        .input(decode_option_env_ref_mut!(decode_MissDetectionConfig))
        .finish()
}


/// Rust → Kotlin name mappings consumed by `KotlinInterfaceGenerator`.
fn shared_kotlin_types() -> KotlinTypeMap {
    KotlinTypeMap::new()
        .with_primitive_builtins()
        .add("String", "String")
        .add("Option<String>", "String")
        .add("Vec<u8>", "ByteArray")
        .add("Option<Vec<u8>>", "ByteArray")
        .add("impl Fn(Sample) + Send + Sync + 'static", "io.zenoh.jni.callbacks.JNISubscriberCallback")
        .add("impl Fn(Query) + Send + Sync + 'static", "io.zenoh.jni.callbacks.JNIQueryableCallback")
        .add("impl Fn(Reply) + Send + Sync + 'static", "io.zenoh.jni.callbacks.JNIGetCallback")
        .add("impl Fn() + Send + Sync + 'static", "io.zenoh.jni.callbacks.JNIOnCloseCallback")
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

fn main() {
    let source = prebindgen::Source::new(zenoh_flat::PREBINDGEN_OUT_DIR);

    // Phase 1: process #[prebindgen] structs from zenoh_flat::structs via a
    // JNI decoder strategy. Each struct registers a type row in the
    // shared TypeRegistry and emits a `decode_<Name>` Rust fn.
    let mut struct_conv = TypesConverter::builder(JniDecoderStruct::new(
        "zenoh_flat::structs",
        "crate::errors::ZResult",
    )
    .java_class_prefix("io/zenoh/jni"))
    .type_registry(shared_bindings())
    .build();

    let struct_items: Vec<_> = source
        .items_all()
        .filter(|(item, loc)| {
            matches!(item, syn::Item::Struct(_)) && loc.file.ends_with("/structs.rs")
        })
        .batching(struct_conv.as_closure())
        .collect();

    let types = struct_conv.into_type_registry();

    // Phase 1b: process the `#[prebindgen]` struct from zenoh_flat::keyexpr
    // (the `KeyExpr` flat handle). Separate pass because its source module
    // path differs from `zenoh_flat::structs`.
    let mut keyexpr_struct_conv = TypesConverter::builder(JniDecoderStruct::new(
        "zenoh_flat::keyexpr",
        "crate::errors::ZResult",
    )
    .java_class_prefix("io/zenoh/jni"))
    .type_registry(types)
    .build();

    let keyexpr_struct_items: Vec<_> = source
        .items_all()
        .filter(|(item, loc)| {
            matches!(item, syn::Item::Struct(_)) && loc.file.ends_with("/keyexpr.rs")
        })
        .batching(keyexpr_struct_conv.as_closure())
        .collect();

    let types = keyexpr_struct_conv.into_type_registry();

    // Phase 2: process #[prebindgen] fns from zenoh_flat::session and
    // zenoh_flat::keyexpr against the now fully-populated type registry,
    // with a JNI try-closure body strategy. Each module gets its own
    // FunctionsConverter pass so that the JNI symbol prefix can target a
    // distinct destination Kotlin object (`JNISessionNative` vs
    // `JNIKeyExprNative`).
    let extra_leading: TokenStream = quote! {
        mut env: jni::JNIEnv,
        _class: jni::objects::JClass
    };
    let extra_attrs: Vec<syn::Attribute> = vec![
        syn::parse_quote!(#[no_mangle]),
        syn::parse_quote!(#[allow(non_snake_case, unused_mut, unused_variables)]),
    ];
    let mut session_conv = FunctionsConverter::builder(JniTryClosureBody::new(
        "crate::errors::ZResult",
        "crate::throw_exception",
    ))
    .source_module("zenoh_flat::session")
    .name_mangler(NameMangler::CamelPrefixSuffix {
        prefix: "Java_io_zenoh_jni_JNISessionNative_".into(),
        suffix: "ViaJNI".into(),
    })
    .extra_leading_params(extra_leading.clone())
    .extra_attrs(extra_attrs.clone())
    .extern_abi(syn::parse_quote!(extern "C"))
    .unsafety(true)
    .type_registry(types.clone())
    .build();

    let session_items: Vec<_> = source
        .items_all()
        .filter(|(item, loc)| {
            matches!(item, syn::Item::Fn(_)) && loc.file.ends_with("/session.rs")
        })
        .batching(session_conv.as_closure())
        .collect();

    let mut keyexpr_conv = FunctionsConverter::builder(JniTryClosureBody::new(
        "crate::errors::ZResult",
        "crate::throw_exception",
    ))
    .source_module("zenoh_flat::keyexpr")
    .name_mangler(NameMangler::CamelPrefixSuffix {
        prefix: "Java_io_zenoh_jni_JNIKeyExprNative_".into(),
        suffix: "ViaJNI".into(),
    })
    .extra_leading_params(extra_leading)
    .extra_attrs(extra_attrs)
    .extern_abi(syn::parse_quote!(extern "C"))
    .unsafety(true)
    .type_registry(types.clone())
    .build();

    let keyexpr_items: Vec<_> = source
        .items_all()
        .filter(|(item, loc)| {
            matches!(item, syn::Item::Fn(_)) && loc.file.ends_with("/keyexpr.rs")
        })
        .batching(keyexpr_conv.as_closure())
        .collect();

    // Pass-through: items that are neither `#[prebindgen]` structs nor fns
    // (e.g. the prebindgen feature-mismatch assertion `const _: () = { ... };`).
    let passthrough = source
        .items_all()
        .filter(|(item, _)| !matches!(item, syn::Item::Fn(_) | syn::Item::Struct(_)));

    let bindings_file = struct_items
        .into_iter()
        .chain(keyexpr_struct_items)
        .chain(session_items)
        .chain(keyexpr_items)
        .chain(passthrough)
        .collect::<prebindgen::collect::Destination>()
        .write("zenoh_flat_jni.rs");

    println!(
        "cargo:warning=Generated bindings at: {}",
        bindings_file.display()
    );

    // Phase 3: Kotlin interface declaration. Walks the same items in a
    // separate pass.
    let struct_names = [
        "HistoryConfig",
        "RecoveryConfig",
        "CacheConfig",
        "MissDetectionConfig",
        "KeyExpr",
    ];
    let mut kotlin_types = shared_kotlin_types();
    for s in &struct_names {
        kotlin_types = kotlin_types
            .add(*s, *s)
            .add(format!("Option<{}>", s), *s);
    }

    let mut session_kotlin = KotlinInterfaceGenerator::builder()
        .output_path("../zenoh-jni/generated-kotlin/io/zenoh/jni/JNISessionNative.kt")
        .package("io.zenoh.jni")
        .class_name("JNISessionNative")
        .throws_class("io.zenoh.exceptions.ZError")
        .init_load("io.zenoh.ZenohLoad")
        .function_suffix("ViaJNI")
        .type_registry(types.clone())
        .kotlin_types(kotlin_types.clone())
        .build();

    for (item, loc) in source.items_all().filter(|(item, loc)| {
        (matches!(item, syn::Item::Struct(_)) && loc.file.ends_with("/structs.rs"))
            || (matches!(item, syn::Item::Fn(_)) && loc.file.ends_with("/session.rs"))
    }) {
        session_kotlin.add_item(&item, &loc);
    }
    session_kotlin
        .write()
        .expect("failed to write generated JNISessionNative.kt");

    let mut keyexpr_kotlin = KotlinInterfaceGenerator::builder()
        .output_path("../zenoh-jni/generated-kotlin/io/zenoh/jni/JNIKeyExprNative.kt")
        .package("io.zenoh.jni")
        .class_name("JNIKeyExprNative")
        .throws_class("io.zenoh.exceptions.ZError")
        .init_load("io.zenoh.ZenohLoad")
        .function_suffix("ViaJNI")
        .type_registry(types)
        .kotlin_types(kotlin_types)
        .build();

    for (item, loc) in source.items_all().filter(|(item, loc)| {
        (matches!(item, syn::Item::Struct(_)) && loc.file.ends_with("/keyexpr.rs"))
            || (matches!(item, syn::Item::Fn(_)) && loc.file.ends_with("/keyexpr.rs"))
    }) {
        keyexpr_kotlin.add_item(&item, &loc);
    }
    keyexpr_kotlin
        .write()
        .expect("failed to write generated JNIKeyExprNative.kt");
}
