use itertools::Itertools;
use zenoh_flat::jni_converter::{ArgDecode, JniForm, ReturnEncode, ReturnForm, TypeBinding};
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

/// Type vocabulary shared across every `JniConverter` build in this crate.
/// Defined once and ingested via `Builder::jni_type_binding(...)` so each
/// generated JNI surface (session, publisher, subscriber, ...) sees the same
/// types without duplicating registrations.
fn shared_bindings() -> JniTypeBinding {
    JniTypeBinding::new()
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

    let mut converter = zenoh_flat::jni_converter::JniConverter::builder()
        .class_prefix("Java_io_zenoh_jni_JNISessionNative_")
        .function_suffix("ViaJNI")
        .source_module("zenoh_flat::session")
        .struct_source_module("zenoh_flat::ext")
        .owned_object("crate::owned_object::OwnedObject")
        .zresult("crate::errors::ZResult")
        .throw_exception("crate::throw_exception")
        .string_decoder("crate::utils::decode_string")
        .byte_array_decoder("crate::utils::decode_byte_array")
        .kotlin_output("../zenoh-jni/generated-kotlin/io/zenoh/jni/JNISessionNative.kt")
        .kotlin_package("io.zenoh.jni")
        .kotlin_class("JNISessionNative")
        .kotlin_throws("io.zenoh.exceptions.ZError")
        .kotlin_init("io.zenoh.ZenohLoad")
        .jni_type_binding(shared_bindings())
        .build();

    let bindings_file =source
        .items_all()
        .batching(converter.as_closure())
        .collect::<prebindgen::collect::Destination>()
        .write("zenoh_flat_jni.rs");

    println!(
        "cargo:warning=Generated bindings at: {}",
        bindings_file.display()
    );

    converter
        .write_kotlin()
        .expect("failed to write generated Kotlin file");
}
