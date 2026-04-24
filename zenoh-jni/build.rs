use itertools::Itertools;

fn main() {
    let source = prebindgen::Source::new(zenoh_flat::PREBINDGEN_OUT_DIR);

    let converter = zenoh_flat::jni_converter::JniConverter::builder()
        .class_prefix("Java_io_zenoh_jni_JNISession_")
        .function_suffix("ViaJNI")
        .source_module("zenoh_flat::session")
        .owned_object("crate::owned_object::OwnedObject")
        .zresult("crate::errors::ZResult")
        .throw_exception("crate::throw_exception")
        .key_expr_decoder("crate::key_expr::decode_jni_key_expr")
        .string_decoder("crate::utils::decode_string")
        .byte_array_decoder("crate::utils::decode_byte_array")
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
        )
        .callback_decoder(
            "Query",
            "crate::sample_callback::process_kotlin_query_callback",
        )
        .callback_decoder(
            "Reply",
            "crate::sample_callback::process_kotlin_reply_callback",
        )
        .struct_decoder("Encoding", "crate::utils::decode_jni_encoding")
        .struct_decoder(
            "HistoryConfig",
            "crate::ext::advanced_subscriber::decode_history_config",
        )
        .struct_decoder(
            "RecoveryConfig",
            "crate::ext::advanced_subscriber::decode_recovery_config",
        )
        .struct_decoder(
            "CacheConfig",
            "crate::ext::advanced_publisher::decode_cache_config",
        )
        .struct_decoder(
            "MissDetectionConfig",
            "crate::ext::advanced_publisher::decode_miss_detection_config",
        )
        .consume_arg("close_session", "session")
        .consume_arg("undeclare_key_expr", "key_expr")
        .return_wrapper(
            "ZenohId",
            "jni::sys::jbyteArray",
            "crate::zenoh_id::zenoh_id_to_byte_array",
            "jni::objects::JByteArray::default().as_raw()",
        )
        .return_wrapper_vec(
            "ZenohId",
            "jni::sys::jobject",
            "crate::zenoh_id::zenoh_ids_to_java_list",
            "jni::objects::JObject::default().as_raw()",
        )
        .build();

    source
        .items_all()
        .batching(converter.into_closure())
        .collect::<prebindgen::collect::Destination>()
        .write("zenoh_flat_jni.rs");
}
