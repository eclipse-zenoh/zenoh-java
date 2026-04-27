use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::quote;

use zenoh_flat::core::{
    primitive_builtins, FunctionsConverter, NameMangler, ReturnEncode, TypeBinding, TypeRegistry,
    TypesConverter,
};
use zenoh_flat::jni::jni_type_helper;
use zenoh_flat::jni::opaque::{opaque_arc_return, opaque_borrow, option_of_jobject};
use zenoh_flat::jni::{JniDecoderStruct, JniTryClosureBody};
use zenoh_flat::kotlin::{KotlinInterfaceGenerator, KotlinTypeMap};

const OWNED_OBJECT: &str = "crate::owned_object::OwnedObject";

/// Wire-side `TypeBinding` registry shared across every JNI surface
/// generated in this crate. Defined once, threaded into the struct-phase
/// converter, then forwarded — together with the auto-registered struct
/// bindings — into the methods phase and the Kotlin generator.
fn shared_bindings() -> TypeRegistry {
    let string = jni_type_helper::jstring(
        "String",
        "crate::utils::decode_string",
    );
    let bytes = jni_type_helper::jbyte_array(
        "Vec<u8>",
        "crate::utils::decode_byte_array",
    );
    let encoding = jni_type_helper::jobject(
        "Encoding",
        "crate::utils::decode_jni_encoding",
    );
    let history_config = jni_type_helper::jobject(
        "HistoryConfig",
        "decode_HistoryConfig",
    );
    let recovery_config = jni_type_helper::jobject(
        "RecoveryConfig",
        "decode_RecoveryConfig",
    );
    let cache_config = jni_type_helper::jobject(
        "CacheConfig",
        "decode_CacheConfig",
    );
    let miss_detection_config = jni_type_helper::jobject(
        "MissDetectionConfig",
        "decode_MissDetectionConfig",
    );
 
    primitive_builtins()
        // Strings & byte arrays.
        .type_binding(string.clone())
        .type_binding(option_of_jobject(&string))
        .type_binding(bytes.clone())
        .type_binding(option_of_jobject(&bytes))
        // Callbacks.
        .type_binding(jni_type_helper::jobject(
            "impl Fn(Sample) + Send + Sync + 'static",
            "crate::sample_callback::process_kotlin_sample_callback",
        ))
        .type_binding(jni_type_helper::jobject(
            "impl Fn(Query) + Send + Sync + 'static",
            "crate::sample_callback::process_kotlin_query_callback",
        ))
        .type_binding(jni_type_helper::jobject(
            "impl Fn(Reply) + Send + Sync + 'static",
            "crate::sample_callback::process_kotlin_reply_callback",
        ))
        .type_binding(jni_type_helper::jobject(
            "impl Fn() + Send + Sync + 'static",
            "crate::sample_callback::process_kotlin_on_close_callback",
        ))
        // Java-enum-shaped types.
        .type_binding(jni_type_helper::jint(
            "CongestionControl",
            "crate::utils::decode_congestion_control",
        ))
        .type_binding(jni_type_helper::jint(
            "Priority",
            "crate::utils::decode_priority",
        ))
        .type_binding(jni_type_helper::jint(
            "Reliability",
            "crate::utils::decode_reliability",
        ))
        .type_binding(jni_type_helper::jint(
            "QueryTarget",
            "crate::utils::decode_query_target",
        ))
        .type_binding(jni_type_helper::jint(
            "ConsolidationMode",
            "crate::utils::decode_consolidation",
        ))
        .type_binding(jni_type_helper::jint(
            "ReplyKeyExpr",
            "crate::utils::decode_reply_key_expr",
        ))
        // KeyExpr by-value: JNI side passes the JNIKeyExpr holder object.
        .type_binding(jni_type_helper::jobject(
            "KeyExpr<'static>",
            "crate::key_expr::decode_jni_key_expr",
        ))
        // Encoding via JObject + custom decoder.
        .type_binding(encoding.clone())
        .type_binding(option_of_jobject(&encoding))
        // Borrows: opaque Arc handles received as `*const T`.
        .type_binding(opaque_borrow("Session", OWNED_OBJECT))
        .type_binding(opaque_borrow("Config", OWNED_OBJECT))
        // Returns: ZenohId / Vec<ZenohId> via custom encoders.
        .type_binding(TypeBinding::output(
            "ZResult<ZenohId>",
            "jni::sys::jbyteArray",
            ReturnEncode::wrapper("crate::zenoh_id::zenoh_id_to_byte_array"),
            "jni::objects::JByteArray::default().as_raw()",
        ))
        .type_binding(TypeBinding::output(
            "ZResult<Vec<ZenohId>>",
            "jni::sys::jobject",
            ReturnEncode::wrapper("crate::zenoh_id::zenoh_ids_to_java_list"),
            "jni::objects::JObject::default().as_raw()",
        ))
        // Returns: opaque Arc handles. Each emits `*const T` and
        // `Arc::into_raw(Arc::new(__result))` with a null default.
        .type_binding(opaque_arc_return("Session"))
        .type_binding(opaque_arc_return("Publisher<'static>"))
        .type_binding(opaque_arc_return("KeyExpr<'static>"))
        .type_binding(opaque_arc_return("Subscriber<()>"))
        .type_binding(opaque_arc_return("Querier<'static>"))
        .type_binding(opaque_arc_return("Queryable<()>"))
        .type_binding(opaque_arc_return("AdvancedSubscriber<()>"))
        .type_binding(opaque_arc_return("AdvancedPublisher<'static>"))
        // Unit returns: ZResult<()> with `()` wire type so the converter
        // treats it as a no-return shape.
        .type_binding(TypeBinding::input_output(
            "ZResult<()>",
            syn::parse_str::<syn::Type>("()").unwrap(),
            None,
            None,
            None,
        ))
        .type_binding(history_config.clone())
        .type_binding(option_of_jobject(&history_config))
        .type_binding(recovery_config.clone())
        .type_binding(option_of_jobject(&recovery_config))
        .type_binding(cache_config.clone())
        .type_binding(option_of_jobject(&cache_config))
        .type_binding(miss_detection_config.clone())
        .type_binding(option_of_jobject(&miss_detection_config))
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
    // JNI decoder strategy. Each struct registers a TypeBinding in the
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
