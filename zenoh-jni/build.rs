use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::quote;

use zenoh_flat::core::{
    primitive_builtins, FunctionsConverter, InlineFn, NameMangler, TypeRegistry, TypesConverter,
};
use zenoh_flat::jni::{JniDecoderStruct, JniTryClosureBody};
use zenoh_flat::kotlin::{KotlinInterfaceGenerator, KotlinTypeMap};

const OWNED_OBJECT: &str = "crate::owned_object::OwnedObject";

fn decode_pure(path: impl AsRef<str>) -> InlineFn {
    let s = path.as_ref().to_string();
    InlineFn::new(move |input: Option<&syn::Ident>| -> TokenStream {
        let input = input.expect("pure decode requires an input ident");
        let p: syn::Path = syn::parse_str(&s).expect("invalid pure decode path");
        quote! { #p(#input)? }
    })
}

fn decode_env_ref(path: impl AsRef<str>) -> InlineFn {
    let s = path.as_ref().to_string();
    InlineFn::new(move |input: Option<&syn::Ident>| -> TokenStream {
        let input = input.expect("env_ref decode requires an input ident");
        let p: syn::Path = syn::parse_str(&s).expect("invalid env_ref decode path");
        quote! { #p(&env, &#input)? }
    })
}

fn decode_env_ref_mut(path: impl AsRef<str>) -> InlineFn {
    let s = path.as_ref().to_string();
    InlineFn::new(move |input: Option<&syn::Ident>| -> TokenStream {
        let input = input.expect("env_ref_mut decode requires an input ident");
        let p: syn::Path = syn::parse_str(&s).expect("invalid env_ref_mut decode path");
        quote! { #p(&mut env, &#input)? }
    })
}

fn decode_option_env_ref(path: impl AsRef<str>) -> InlineFn {
    let s = path.as_ref().to_string();
    InlineFn::new(move |input: Option<&syn::Ident>| -> TokenStream {
        let input = input.expect("option decode requires an input ident");
        let p: syn::Path = syn::parse_str(&s).expect("invalid option decode path");
        quote! {
            if !#input.is_null() {
                Some(#p(&env, &#input)?)
            } else {
                None
            }
        }
    })
}

fn decode_option_env_ref_mut(path: impl AsRef<str>) -> InlineFn {
    let s = path.as_ref().to_string();
    InlineFn::new(move |input: Option<&syn::Ident>| -> TokenStream {
        let input = input.expect("option decode requires an input ident");
        let p: syn::Path = syn::parse_str(&s).expect("invalid option decode path");
        quote! {
            if !#input.is_null() {
                Some(#p(&mut env, &#input)?)
            } else {
                None
            }
        }
    })
}

fn decode_owned_raw(owned_object: impl AsRef<str>) -> InlineFn {
    let owned = owned_object.as_ref().to_string();
    InlineFn::new(move |input: Option<&syn::Ident>| -> TokenStream {
        let input = input.expect("opaque borrow decode requires an input ident");
        let p: syn::Path = syn::parse_str(&owned).expect("invalid owned object path");
        quote! { #p::from_raw(#input) }
    })
}

fn encode_wrapper(path: impl AsRef<str>, default_expr: impl AsRef<str>) -> InlineFn {
    let s = path.as_ref().to_string();
    let default = default_expr.as_ref().to_string();
    InlineFn::new(move |output: Option<&syn::Ident>| -> TokenStream {
        let p: syn::Path = syn::parse_str(&s).expect("invalid wrapper encode path");
        let default_expr: syn::Expr =
            syn::parse_str(&default).expect("invalid wrapper encode default expr");
        match output {
            Some(output) => quote! { #p(&mut env, #output)? },
            None => quote! { #default_expr },
        }
    })
}

fn encode_arc_into_raw(default_expr: impl AsRef<str>) -> InlineFn {
    let default = default_expr.as_ref().to_string();
    InlineFn::new(move |output: Option<&syn::Ident>| -> TokenStream {
        let default_expr: syn::Expr =
            syn::parse_str(&default).expect("invalid Arc encode default expr");
        match output {
            Some(output) => {
                quote! { std::sync::Arc::into_raw(std::sync::Arc::new(#output)) }
            }
            None => quote! { #default_expr },
        }
    })
}

/// Wire-side `TypeRegistry` shared across every JNI surface
/// generated in this crate. Defined once, threaded into the struct-phase
/// converter, then forwarded — together with the auto-registered struct
/// bindings — into the methods phase and the Kotlin generator.
fn shared_bindings() -> TypeRegistry {
    primitive_builtins()
        // Strings & byte arrays.
        .type_pair("String", "jni::objects::JString")
        .input(decode_env_ref_mut("crate::utils::decode_string"))
        .type_pair("Option<String>", "jni::objects::JString")
        .input(decode_option_env_ref_mut("crate::utils::decode_string"))
        .type_pair("Vec<u8>", "jni::objects::JByteArray")
        .input(decode_env_ref("crate::utils::decode_byte_array"))
        .type_pair("Option<Vec<u8>>", "jni::objects::JByteArray")
        .input(decode_option_env_ref("crate::utils::decode_byte_array"))
        // Callbacks.
        .type_pair("impl Fn(Sample) + Send + Sync + 'static", "jni::objects::JObject")
        .input(decode_env_ref_mut("crate::sample_callback::process_kotlin_sample_callback"))
        .type_pair("impl Fn(Query) + Send + Sync + 'static", "jni::objects::JObject")
        .input(decode_env_ref_mut("crate::sample_callback::process_kotlin_query_callback"))
        .type_pair("impl Fn(Reply) + Send + Sync + 'static", "jni::objects::JObject")
        .input(decode_env_ref_mut("crate::sample_callback::process_kotlin_reply_callback"))
        .type_pair("impl Fn() + Send + Sync + 'static", "jni::objects::JObject")
        .input(decode_env_ref_mut("crate::sample_callback::process_kotlin_on_close_callback"))
        // Java-enum-shaped types.
        .type_pair("CongestionControl", "jni::sys::jint")
        .input(decode_pure("crate::utils::decode_congestion_control"))
        .type_pair("Priority", "jni::sys::jint")
        .input(decode_pure("crate::utils::decode_priority"))
        .type_pair("Reliability", "jni::sys::jint")
        .input(decode_pure("crate::utils::decode_reliability"))
        .type_pair("QueryTarget", "jni::sys::jint")
        .input(decode_pure("crate::utils::decode_query_target"))
        .type_pair("ConsolidationMode", "jni::sys::jint")
        .input(decode_pure("crate::utils::decode_consolidation"))
        .type_pair("ReplyKeyExpr", "jni::sys::jint")
        .input(decode_pure("crate::utils::decode_reply_key_expr"))
        // KeyExpr by-value: JNI side passes the JNIKeyExpr holder object.
        .type_pair("KeyExpr<'static>", "jni::objects::JObject")
        .input(decode_env_ref_mut("crate::key_expr::decode_jni_key_expr"))
        // Encoding via JObject + custom decoder.
        .type_pair("Encoding", "jni::objects::JObject")
        .input(decode_env_ref_mut("crate::utils::decode_jni_encoding"))
        .type_pair("Option<Encoding>", "jni::objects::JObject")
        .input(decode_option_env_ref_mut("crate::utils::decode_jni_encoding"))
        // Borrows: opaque Arc handles received as `*const T`.
        .type_pair("&Session", "*const Session")
        .input(decode_owned_raw(OWNED_OBJECT))
        .type_pair("&Config", "*const Config")
        .input(decode_owned_raw(OWNED_OBJECT))
        // Returns: ZenohId / Vec<ZenohId> via custom encoders.
        .type_pair("ZResult<ZenohId>", "jni::sys::jbyteArray")
        .output(encode_wrapper("crate::zenoh_id::zenoh_id_to_byte_array", "jni::objects::JByteArray::default().as_raw()"))
        .type_pair("ZResult<Vec<ZenohId>>", "jni::sys::jobject")
        .output(encode_wrapper("crate::zenoh_id::zenoh_ids_to_java_list", "jni::objects::JObject::default().as_raw()"))
        // Returns: opaque Arc handles.
        .type_pair("ZResult<Session>", "*const Session")
        .output(encode_arc_into_raw("std::ptr::null()"))
        .type_pair("ZResult<Publisher<'static>>", "*const Publisher<'static>")
        .output(encode_arc_into_raw("std::ptr::null()"))
        .type_pair("ZResult<KeyExpr<'static>>", "*const KeyExpr<'static>")
        .output(encode_arc_into_raw("std::ptr::null()"))
        .type_pair("ZResult<Subscriber<()>>", "*const Subscriber<()>")
        .output(encode_arc_into_raw("std::ptr::null()"))
        .type_pair("ZResult<Querier<'static>>", "*const Querier<'static>")
        .output(encode_arc_into_raw("std::ptr::null()"))
        .type_pair("ZResult<Queryable<()>>", "*const Queryable<()>")
        .output(encode_arc_into_raw("std::ptr::null()"))
        .type_pair("ZResult<AdvancedSubscriber<()>>", "*const AdvancedSubscriber<()>")
        .output(encode_arc_into_raw("std::ptr::null()"))
        .type_pair("ZResult<AdvancedPublisher<'static>>", "*const AdvancedPublisher<'static>")
        .output(encode_arc_into_raw("std::ptr::null()"))
        // Unit returns: ZResult<()> with `()` wire type so the converter treats it as a no-return shape.
        .type_pair("ZResult<()>", "()")
        // Structs from ext.rs and nullable wrappers.
        .type_pair("HistoryConfig", "jni::objects::JObject")
        .input(decode_env_ref_mut("decode_HistoryConfig"))
        .type_pair("Option<HistoryConfig>", "jni::objects::JObject")
        .input(decode_option_env_ref_mut("decode_HistoryConfig"))
        .type_pair("RecoveryConfig", "jni::objects::JObject")
        .input(decode_env_ref_mut("decode_RecoveryConfig"))
        .type_pair("Option<RecoveryConfig>", "jni::objects::JObject")
        .input(decode_option_env_ref_mut("decode_RecoveryConfig"))
        .type_pair("CacheConfig", "jni::objects::JObject")
        .input(decode_env_ref_mut("decode_CacheConfig"))
        .type_pair("Option<CacheConfig>", "jni::objects::JObject")
        .input(decode_option_env_ref_mut("decode_CacheConfig"))
        .type_pair("MissDetectionConfig", "jni::objects::JObject")
        .input(decode_env_ref_mut("decode_MissDetectionConfig"))
        .type_pair("Option<MissDetectionConfig>", "jni::objects::JObject")
        .input(decode_option_env_ref_mut("decode_MissDetectionConfig"))
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
        .add("KeyExpr<'static>", "io.zenoh.jni.JNIKeyExpr")
        .add("Encoding", "io.zenoh.jni.JNIEncoding")
        .add("Option<Encoding>", "io.zenoh.jni.JNIEncoding")
        .add("&Session", "Long")
        .add("&Config", "Long")
        .add("ZResult<ZenohId>", "ByteArray")
        .add("ZResult<Vec<ZenohId>>", "List<ByteArray>")
        .add("ZResult<Session>", "Long")
        .add("ZResult<Publisher<'static>>", "Long")
        .add("ZResult<KeyExpr<'static>>", "Long")
        .add("ZResult<Subscriber<()>>", "Long")
        .add("ZResult<Querier<'static>>", "Long")
        .add("ZResult<Queryable<()>>", "Long")
        .add("ZResult<AdvancedSubscriber<()>>", "Long")
        .add("ZResult<AdvancedPublisher<'static>>", "Long")
}

fn main() {
    let source = prebindgen::Source::new(zenoh_flat::PREBINDGEN_OUT_DIR);

    // Phase 1: process #[prebindgen] structs from zenoh_flat::ext via a
    // JNI decoder strategy. Each struct registers a type row in the
    // shared TypeRegistry and emits a `decode_<Name>` Rust fn.
    let mut struct_conv = TypesConverter::builder(JniDecoderStruct::new(
        "zenoh_flat::ext",
        "crate::errors::ZResult",
    ))
    .type_registry(shared_bindings())
    .build();

    let struct_items: Vec<_> = source
        .items_all()
        .filter(|(item, loc)| {
            matches!(item, syn::Item::Struct(_)) && loc.file.ends_with("/ext.rs")
        })
        .batching(struct_conv.as_closure())
        .collect();

    let types = struct_conv.into_type_registry();

    // Phase 2: process #[prebindgen] fns from zenoh_flat::session against
    // the now fully-populated type registry, with a JNI try-closure body
    // strategy.
    let extra_leading: TokenStream = quote! {
        mut env: jni::JNIEnv,
        _class: jni::objects::JClass
    };
    let extra_attrs: Vec<syn::Attribute> = vec![
        syn::parse_quote!(#[no_mangle]),
        syn::parse_quote!(#[allow(non_snake_case, unused_mut, unused_variables)]),
    ];
    let mut method_conv = FunctionsConverter::builder(JniTryClosureBody::new(
        "crate::errors::ZResult",
        "crate::throw_exception",
    ))
    .source_module("zenoh_flat::session")
    .name_mangler(NameMangler::CamelPrefixSuffix {
        prefix: "Java_io_zenoh_jni_JNISessionNative_".into(),
        suffix: "ViaJNI".into(),
    })
    .extra_leading_params(extra_leading)
    .extra_attrs(extra_attrs)
    .extern_abi(syn::parse_quote!(extern "C"))
    .unsafety(true)
    .type_registry(types.clone())
    .build();

    let method_items: Vec<_> = source
        .items_all()
        .filter(|(item, loc)| {
            matches!(item, syn::Item::Fn(_)) && loc.file.ends_with("/session.rs")
        })
        .batching(method_conv.as_closure())
        .collect();

    // Pass-through: items that are neither `#[prebindgen]` structs nor fns
    // (e.g. the prebindgen feature-mismatch assertion `const _: () = { ... };`).
    let passthrough = source
        .items_all()
        .filter(|(item, _)| !matches!(item, syn::Item::Fn(_) | syn::Item::Struct(_)));

    let bindings_file = struct_items
        .into_iter()
        .chain(method_items)
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
    ];
    let mut kotlin_types = shared_kotlin_types();
    for s in &struct_names {
        kotlin_types = kotlin_types
            .add(*s, *s)
            .add(format!("Option<{}>", s), *s);
    }

    let mut kotlin = KotlinInterfaceGenerator::builder()
        .output_path("../zenoh-jni/generated-kotlin/io/zenoh/jni/JNISessionNative.kt")
        .package("io.zenoh.jni")
        .class_name("JNISessionNative")
        .throws_class("io.zenoh.exceptions.ZError")
        .init_load("io.zenoh.ZenohLoad")
        .function_suffix("ViaJNI")
        .type_registry(types)
        .kotlin_types(kotlin_types)
        .build();

    for (item, loc) in source.items_all().filter(|(item, loc)| {
        (matches!(item, syn::Item::Struct(_)) && loc.file.ends_with("/ext.rs"))
            || (matches!(item, syn::Item::Fn(_)) && loc.file.ends_with("/session.rs"))
    }) {
        kotlin.add_item(&item, &loc);
    }
    kotlin.write().expect("failed to write generated Kotlin file");
}
