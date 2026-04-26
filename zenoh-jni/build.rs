use itertools::Itertools;
use zenoh_flat::jni_converter::{
    ArgDecode, JniForm, JniMethodsConverter, JniStructConverter, ReturnEncode, ReturnForm,
    TypeBinding,
};
use zenoh_flat::jni_type_binding::JniTypeBinding;

fn enum_binding(name: &str, decoder: &str) -> TypeBinding {
    TypeBinding::new(name).consume(JniForm::new("jni::sys::jint", "Int", ArgDecode::pure(decoder)))
}

fn jobject_consume(name: &str, decoder: &str, kotlin: &str) -> TypeBinding {
    TypeBinding::new(name).kotlin(kotlin).consume(JniForm::new(
        "jni::objects::JObject",
        "JObject",
        ArgDecode::env_ref_mut(decoder),
    ))
}

/// Type vocabulary shared across every JNI surface generated in this crate.
/// Defined once, threaded into the struct-phase converter (so struct field
/// decoders can resolve enum types), then forwarded — together with the
/// auto-registered struct bindings — into the methods-phase converter.
fn shared_bindings() -> JniTypeBinding {
    JniTypeBinding::new()
        .type_binding(
            TypeBinding::new("String").consume(JniForm::new(
                "jni::objects::JString",
                "String",
                ArgDecode::env_ref_mut("crate::utils::decode_string"),
            )),
        )
        // `Vec<u8>` is keyed under the synthetic name "VecU8" — the
        // methods-phase classifier looks it up explicitly when it sees
        // `Vec<u8>`.
        .type_binding(
            TypeBinding::new("VecU8").consume(JniForm::new(
                "jni::objects::JByteArray",
                "ByteArray",
                ArgDecode::env_ref("crate::utils::decode_byte_array"),
            )),
        )
        .type_binding(jobject_consume(
            "impl Fn(Sample) + Send + Sync + 'static",
            "crate::sample_callback::process_kotlin_sample_callback",
            "io.zenoh.jni.callbacks.JNISubscriberCallback",
        ))
        .type_binding(jobject_consume(
            "impl Fn(Query) + Send + Sync + 'static",
            "crate::sample_callback::process_kotlin_query_callback",
            "io.zenoh.jni.callbacks.JNIQueryableCallback",
        ))
        .type_binding(jobject_consume(
            "impl Fn(Reply) + Send + Sync + 'static",
            "crate::sample_callback::process_kotlin_reply_callback",
            "io.zenoh.jni.callbacks.JNIGetCallback",
        ))
        .type_binding(jobject_consume(
            "impl Fn() + Send + Sync + 'static",
            "crate::sample_callback::process_kotlin_on_close_callback",
            "io.zenoh.jni.callbacks.JNIOnCloseCallback",
        ))
        .type_binding(enum_binding(
            "CongestionControl",
            "crate::utils::decode_congestion_control",
        ))
        .type_binding(enum_binding("Priority", "crate::utils::decode_priority"))
        .type_binding(enum_binding("Reliability", "crate::utils::decode_reliability"))
        .type_binding(enum_binding(
            "QueryTarget",
            "crate::utils::decode_query_target",
        ))
        .type_binding(enum_binding(
            "ConsolidationMode",
            "crate::utils::decode_consolidation",
        ))
        .type_binding(enum_binding(
            "ReplyKeyExpr",
            "crate::utils::decode_reply_key_expr",
        ))
        // KeyExpr by-value: the JNI side passes `Arc::into_raw(Arc::new(KeyExpr))`
        // as a raw pointer; the wrapper reconstructs the Arc, clones the inner
        // KeyExpr, and drops the Arc at end of scope. The full path is required
        // so the generated `*const T` parameter type resolves at the include site.
        .type_binding(
            TypeBinding::new("KeyExpr").consume(
                JniForm::new(
                    "*const zenoh::key_expr::KeyExpr<'static>",
                    "Long",
                    ArgDecode::ConsumeArc,
                )
                .pointer_param(true),
            ),
        )
        .type_binding(jobject_consume(
            "Encoding",
            "crate::utils::decode_jni_encoding",
            "JNIEncoding",
        ))
        .type_binding(
            TypeBinding::new("ZenohId")
                .returns(
                    ReturnForm::new(
                        "jni::sys::jbyteArray",
                        ReturnEncode::wrapper("crate::zenoh_id::zenoh_id_to_byte_array"),
                        "jni::objects::JByteArray::default().as_raw()",
                    )
                    .kotlin("ByteArray"),
                )
                .returns_vec(
                    ReturnForm::new(
                        "jni::sys::jobject",
                        ReturnEncode::wrapper("crate::zenoh_id::zenoh_ids_to_java_list"),
                        "jni::objects::JObject::default().as_raw()",
                    )
                    .kotlin("List<ByteArray>"),
                ),
        )
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

    let types = struct_conv.into_jni_type_binding();

    // Phase 2: process #[prebindgen] fns from zenoh_flat::session against
    // the now fully-populated type registry.
    let mut method_conv = JniMethodsConverter::builder()
        .class_prefix("Java_io_zenoh_jni_JNISessionNative_")
        .function_suffix("ViaJNI")
        .source_module("zenoh_flat::session")
        .owned_object("crate::owned_object::OwnedObject")
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
    // The two converters intentionally panic on the wrong item kind, so any
    // such items must bypass them and land directly in the destination.
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
