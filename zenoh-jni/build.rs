use itertools::Itertools;

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
        .enum_decoder(
            "CongestionControl",
            "crate::utils::decode_congestion_control",
        )
        .enum_decoder("Priority", "crate::utils::decode_priority")
        .enum_decoder("Reliability", "crate::utils::decode_reliability")
        .enum_decoder("QueryTarget", "crate::utils::decode_query_target")
        .enum_decoder("ConsolidationMode", "crate::utils::decode_consolidation")
        .enum_decoder("ReplyKeyExpr", "crate::utils::decode_reply_key_expr")
        .callback_decoder(
            "Sample",
            "crate::sample_callback::process_kotlin_sample_callback",
            "io.zenoh.jni.callbacks.JNISubscriberCallback",
        )
        .callback_decoder(
            "Query",
            "crate::sample_callback::process_kotlin_query_callback",
            "io.zenoh.jni.callbacks.JNIQueryableCallback",
        )
        .callback_decoder(
            "Reply",
            "crate::sample_callback::process_kotlin_reply_callback",
            "io.zenoh.jni.callbacks.JNIGetCallback",
        )
        .callback_decoder(
            "()",
            "crate::sample_callback::process_kotlin_on_close_callback",
            "io.zenoh.jni.callbacks.JNIOnCloseCallback",
        )
        .struct_decoder(
            "KeyExpr",
            "crate::key_expr::decode_jni_key_expr",
            "JNIKeyExpr",
        )
        .struct_decoder(
            "Encoding",
            "crate::utils::decode_jni_encoding",
            "JNIEncoding",
        )
        .return_wrapper(
            "ZenohId",
            "jni::sys::jbyteArray",
            "crate::zenoh_id::zenoh_id_to_byte_array",
            "jni::objects::JByteArray::default().as_raw()",
            "ByteArray",
        )
        .return_wrapper_vec(
            "ZenohId",
            "jni::sys::jobject",
            "crate::zenoh_id::zenoh_ids_to_java_list",
            "jni::objects::JObject::default().as_raw()",
            "List<ByteArray>",
        )
        .build();

    source
        .items_all()
        .batching(converter.as_closure())
        .collect::<prebindgen::collect::Destination>()
        .write("zenoh_flat_jni.rs");

    converter
        .write_kotlin()
        .expect("failed to write generated Kotlin file");
}
