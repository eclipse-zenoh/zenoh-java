use itertools::Itertools;
use quote::quote;
use zenoh_flat::jni_converter::{
    InlineFn, JniMethodsConverter, JniStructConverter, TypeBinding,
};
use zenoh_flat::jni_type_binding::{JniTypeBinding, ReturnEncode};

const OWNED_OBJECT: &str = "crate::owned_object::OwnedObject";

fn enum_param(name: &str, decoder: &str) -> TypeBinding {
    TypeBinding::param(name, "Int", "jni::sys::jint", InlineFn::pure(decoder))
        .enum_field_decoder(decoder)
}

fn jobject_param(name: &str, decoder: &str, kotlin: &str) -> TypeBinding {
    TypeBinding::param(
        name,
        kotlin,
        "jni::objects::JObject",
        InlineFn::env_ref_mut(decoder),
    )
}

fn wrapped_return(rust_type: &str, kotlin: &str, jni_type: &str, wrapper: &str, default: &str) -> TypeBinding {
    TypeBinding::returns(
        rust_type,
        kotlin,
        jni_type,
        ReturnEncode::wrapper(wrapper),
        default,
    )
}

/// Type vocabulary shared across every JNI surface generated in this crate.
/// Defined once, threaded into the struct-phase converter, then forwarded —
/// together with the auto-registered struct bindings — into the methods phase.
fn shared_bindings() -> JniTypeBinding {
    JniTypeBinding::new()
        // Strings & byte arrays.
        .type_binding(TypeBinding::param(
            "String",
            "String",
            "jni::objects::JString",
            InlineFn::env_ref_mut("crate::utils::decode_string"),
        ))
        .type_binding(TypeBinding::param(
            "Vec<u8>",
            "ByteArray",
            "jni::objects::JByteArray",
            InlineFn::env_ref("crate::utils::decode_byte_array"),
        ))
        // Callbacks.
        .type_binding(jobject_param(
            "impl Fn(Sample) + Send + Sync + 'static",
            "crate::sample_callback::process_kotlin_sample_callback",
            "io.zenoh.jni.callbacks.JNISubscriberCallback",
        ))
        .type_binding(jobject_param(
            "impl Fn(Query) + Send + Sync + 'static",
            "crate::sample_callback::process_kotlin_query_callback",
            "io.zenoh.jni.callbacks.JNIQueryableCallback",
        ))
        .type_binding(jobject_param(
            "impl Fn(Reply) + Send + Sync + 'static",
            "crate::sample_callback::process_kotlin_reply_callback",
            "io.zenoh.jni.callbacks.JNIGetCallback",
        ))
        .type_binding(jobject_param(
            "impl Fn() + Send + Sync + 'static",
            "crate::sample_callback::process_kotlin_on_close_callback",
            "io.zenoh.jni.callbacks.JNIOnCloseCallback",
        ))
        // Java-enum-shaped types.
        .type_binding(enum_param("CongestionControl", "crate::utils::decode_congestion_control"))
        .type_binding(enum_param("Priority", "crate::utils::decode_priority"))
        .type_binding(enum_param("Reliability", "crate::utils::decode_reliability"))
        .type_binding(enum_param("QueryTarget", "crate::utils::decode_query_target"))
        .type_binding(enum_param("ConsolidationMode", "crate::utils::decode_consolidation"))
        .type_binding(enum_param("ReplyKeyExpr", "crate::utils::decode_reply_key_expr"))
        // KeyExpr by-value: JNI side passes `Arc::into_raw(Arc::new(KeyExpr))`
        // as a raw pointer; the wrapper reconstructs the Arc, clones the inner
        // KeyExpr, and drops the Arc at end of scope. The full path is required
        // so the generated `*const T` parameter type resolves at the include site.
        .type_binding(TypeBinding::param(
            "KeyExpr<'static>",
            "Long",
            "*const zenoh::key_expr::KeyExpr<'static>",
            InlineFn::new(|input| quote! { (*std::sync::Arc::from_raw(#input)).clone() }),
        ))
        // Encoding via JObject + custom decoder.
        .type_binding(jobject_param(
            "Encoding",
            "crate::utils::decode_jni_encoding",
            "io.zenoh.jni.JNIEncoding",
        ))
        // Borrows: opaque Arc handles received as `*const T` and re-borrowed
        // via OwnedObject::from_raw. The `&` prefix on the row's key tells
        // the converter to pass `&name` to the wrapped fn.
        .type_binding(TypeBinding::opaque_borrow("Session", OWNED_OBJECT))
        .type_binding(TypeBinding::opaque_borrow("Config", OWNED_OBJECT))
        // Returns: ZenohId / Vec<ZenohId> via custom encoders.
        .type_binding(wrapped_return(
            "ZResult<ZenohId>",
            "ByteArray",
            "jni::sys::jbyteArray",
            "crate::zenoh_id::zenoh_id_to_byte_array",
            "jni::objects::JByteArray::default().as_raw()",
        ))
        .type_binding(wrapped_return(
            "ZResult<Vec<ZenohId>>",
            "List<ByteArray>",
            "jni::sys::jobject",
            "crate::zenoh_id::zenoh_ids_to_java_list",
            "jni::objects::JObject::default().as_raw()",
        ))
        // Returns: opaque Arc handles. Each emits `*const T` and
        // `Arc::into_raw(Arc::new(__result))` with a null default.
        .type_binding(TypeBinding::opaque_arc_return("Session"))
        .type_binding(TypeBinding::opaque_arc_return("Publisher<'static>"))
        .type_binding(TypeBinding::opaque_arc_return("KeyExpr<'static>"))
        .type_binding(TypeBinding::opaque_arc_return("Subscriber<()>"))
        .type_binding(TypeBinding::opaque_arc_return("Querier<'static>"))
        .type_binding(TypeBinding::opaque_arc_return("Queryable<()>"))
        .type_binding(TypeBinding::opaque_arc_return("AdvancedSubscriber<()>"))
        .type_binding(TypeBinding::opaque_arc_return("AdvancedPublisher<'static>"))
}

/// Build `Option<X>` rows for every X that can appear under `Option<...>` in
/// the wrapped fn signatures. Must be called after struct-converter has
/// registered the auto-generated struct rows.
fn add_option_rows(types: JniTypeBinding) -> JniTypeBinding {
    let inner_keys = [
        "String",
        "Vec<u8>",
        "Encoding",
        "HistoryConfig",
        "RecoveryConfig",
        "CacheConfig",
        "MissDetectionConfig",
    ];
    let mut out = types;
    for key in inner_keys {
        let inner = out
            .type_by_key(key)
            .unwrap_or_else(|| panic!("add_option_rows: missing inner row `{}`", key))
            .clone();
        let opt = TypeBinding::option_of(&inner);
        out = out.type_binding(opt);
    }
    out
}

fn main() {
    let source = prebindgen::Source::new(zenoh_flat::PREBINDGEN_OUT_DIR);

    // Phase 1: process #[prebindgen] structs from zenoh_flat::ext.
    // Each struct adds a TypeBinding (and a Kotlin data class) to the
    // shared JniTypeBinding that we forward to the methods converter.
    let mut struct_conv = JniStructConverter::builder()
        .source_module("zenoh_flat::ext")
        .zresult("crate::errors::ZResult")
        .jni_type_binding(shared_bindings())
        .build();

    let struct_items: Vec<_> = source
        .items_all()
        .filter(|(item, loc)| {
            matches!(item, syn::Item::Struct(_)) && loc.file.ends_with("/ext.rs")
        })
        .batching(struct_conv.as_closure())
        .collect();

    let types = add_option_rows(struct_conv.into_jni_type_binding());

    // Phase 2: process #[prebindgen] fns from zenoh_flat::session against
    // the now fully-populated type registry.
    let mut method_conv = JniMethodsConverter::builder()
        .class_prefix("Java_io_zenoh_jni_JNISessionNative_")
        .function_suffix("ViaJNI")
        .source_module("zenoh_flat::session")
        .zresult("crate::errors::ZResult")
        .throw_exception("crate::throw_exception")
        .kotlin_output("../zenoh-jni/generated-kotlin/io/zenoh/jni/JNISessionNative.kt")
        .kotlin_package("io.zenoh.jni")
        .kotlin_class("JNISessionNative")
        .kotlin_throws("io.zenoh.exceptions.ZError")
        .kotlin_init("io.zenoh.ZenohLoad")
        .jni_type_binding(types)
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

    method_conv
        .write_kotlin()
        .expect("failed to write generated Kotlin file");
}
