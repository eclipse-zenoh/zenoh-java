//! Build script — declarative configuration of the prebindgen-ext
//! pipeline. Reads top-to-bottom as:
//!   1. Configure `JniExt` (Rust crate paths + Kotlin output paths +
//!      per-type rules: opaque handles, jint enums, custom decoders,
//!      callback overrides, data-class names, `impl Into<T>` arms).
//!   2. Scan `zenoh_flat`'s prebindgen source and write the generated
//!      Rust bindings (`zenoh_flat_jni.rs`).
//!   3. Write all Kotlin output (`JNI*Callback.kt`, `NativeHandle.kt`,
//!      typed-handle classes, `JNIWrappers.kt`).

use prebindgen_ext::core::prebindgen_ext::IntoSource;
use prebindgen_ext::core::registry::Registry;
use prebindgen_ext::jni::JniExt;

fn main() {
    let jni = JniExt::new()
        .source_module("zenoh_flat")
        .zresult("crate::errors::ZResult")
        .throw_macro("crate::throw_exception")
        .zerror_macro("zerror")
        .java_class_prefix("io/zenoh/jni")
        .jni_class_path("Java_io_zenoh_jni_JNINative")
        .jni_method_suffix("ViaJNI")
        .kotlin_callback_package("io.zenoh.jni.callbacks")
        .kotlin_callback_dir("../zenoh-jni-runtime/src/commonMain/kotlin/io/zenoh/jni/callbacks")
        // ── Opaque handles — `opaque_handle` configures: jlong wire
        // (input + output), `Box::into_raw`/`Box::from_raw` lifecycle,
        // `instanceof` dispatch class, and the Kotlin parameter-type
        // name. The Kotlin `.kt` file is assumed to be hand-maintained
        // by default; chain `.with_methods(...)` (auto-generated class
        // with promoted instance methods) or `.emit_kotlin_class()`
        // (auto-generated shell class) to switch to auto-emission.
        .opaque_handle("Session", "io.zenoh.jni.JNISession")
        .opaque_handle("Config", "io.zenoh.jni.JNIConfig")
        .opaque_handle("ZKeyExpr<'static>", "io.zenoh.jni.JNIKeyExpr")
        .opaque_handle("Publisher<'static>", "io.zenoh.jni.JNIPublisher")
        .with_methods(["put_publisher", "delete_publisher"])
        .opaque_handle("Subscriber<()>", "io.zenoh.jni.JNISubscriber")
        .emit_kotlin_class()
        .opaque_handle("Querier<'static>", "io.zenoh.jni.JNIQuerier")
        .opaque_handle("Queryable<()>", "io.zenoh.jni.JNIQueryable")
        .emit_kotlin_class()
        .opaque_handle(
            "AdvancedSubscriber<()>",
            "io.zenoh.jni.JNIAdvancedSubscriber",
        )
        .opaque_handle(
            "AdvancedPublisher<'static>",
            "io.zenoh.jni.JNIAdvancedPublisher",
        )
        .opaque_handle("MatchingListener", "io.zenoh.jni.JNIMatchingListener")
        .emit_kotlin_class()
        .opaque_handle("SampleMissListener", "io.zenoh.jni.JNISampleMissListener")
        .emit_kotlin_class()
        // ── jint-encoded enums — sugar over `input_decoder` for the
        // common `jint → enum` pattern.
        .jint_enum(
            "CongestionControl",
            "crate::utils::decode_congestion_control",
        )
        .jint_enum("Priority", "crate::utils::decode_priority")
        .jint_enum("Reliability", "crate::utils::decode_reliability")
        .jint_enum("QueryTarget", "crate::utils::decode_query_target")
        .jint_enum("ConsolidationMode", "crate::utils::decode_consolidation")
        .jint_enum("ReplyKeyExpr", "crate::utils::decode_reply_key_expr")
        // ── Value-shaped custom converters.
        .input_decoder(
            "Encoding",
            "jni::objects::JObject",
            "crate::utils::decode_jni_encoding(env, &v)?",
        )
        .input_decoder(
            "Option<Encoding>",
            "jni::objects::JObject",
            "if !v.is_null() { Some(crate::utils::decode_jni_encoding(env, &v)?) } else { None }",
        )
        .output_encoder(
            "SetIntersectionLevel",
            "jni::sys::jint",
            "v as jni::sys::jint",
        )
        .output_encoder(
            "ZenohId",
            "jni::sys::jbyteArray",
            "crate::zenoh_id::zenoh_id_to_byte_array(env, v)?",
        )
        .output_encoder(
            "Vec<ZenohId>",
            "jni::sys::jobject",
            "crate::zenoh_id::zenoh_ids_to_java_list(env, v)?",
        )
        // ── Manual callback overrides — replaces the auto-generated
        // `process_kotlin_*_callback` dispatcher with a hand-written
        // one and reroutes the Kotlin FQN.
        .callback_input(
            "impl Fn(Query) + Send + Sync + 'static",
            "crate::sample_callback::process_kotlin_query_callback",
            "io.zenoh.jni.callbacks.JNIQueryableCallback",
        )
        .callback_input(
            "impl Fn(Reply) + Send + Sync + 'static",
            "crate::sample_callback::process_kotlin_reply_callback",
            "io.zenoh.jni.callbacks.JNIGetCallback",
        )
        .callback_kotlin_name(
            "impl Fn() + Send + Sync + 'static",
            "io.zenoh.jni.callbacks.JNIOnCloseCallback",
        )
        // ── Kotlin type names for value types that have automatic JNI
        // converters (data classes, primitive aliases, callback param
        // types whose Kotlin form is hand-maintained).
        .kotlin_value_type("String", "String")
        .kotlin_value_type("Option<String>", "String")
        .kotlin_value_type("Vec<u8>", "ByteArray")
        .kotlin_value_type("Option<Vec<u8>>", "ByteArray")
        .kotlin_value_type("CongestionControl", "Int")
        .kotlin_value_type("Priority", "Int")
        .kotlin_value_type("Reliability", "Int")
        .kotlin_value_type("QueryTarget", "Int")
        .kotlin_value_type("ConsolidationMode", "Int")
        .kotlin_value_type("ReplyKeyExpr", "Int")
        .kotlin_value_type("Option<ZKeyExpr<'static>>", "Long")
        .kotlin_value_type("ZResult<SetIntersectionLevel>", "Int")
        .kotlin_value_type("Encoding", "io.zenoh.jni.JNIEncoding")
        .kotlin_value_type("Option<Encoding>", "io.zenoh.jni.JNIEncoding")
        .kotlin_value_type("&Session", "Long")
        .kotlin_value_type("&Config", "Long")
        .kotlin_value_type("ZResult<ZenohId>", "ByteArray")
        .kotlin_value_type("ZResult<Vec<ZenohId>>", "List<ByteArray>")
        .kotlin_value_type("ZResult<Session>", "Long")
        .kotlin_value_type("ZResult<Publisher<'static>>", "Long")
        .kotlin_value_type("ZResult<Subscriber<()>>", "Long")
        .kotlin_value_type("ZResult<Querier<'static>>", "Long")
        .kotlin_value_type("ZResult<Queryable<()>>", "Long")
        .kotlin_value_type("ZResult<AdvancedSubscriber<()>>", "Long")
        .kotlin_value_type("ZResult<AdvancedPublisher<'static>>", "Long")
        .kotlin_value_type("ZResult<bool>", "Boolean")
        // Data classes — hand-maintained in JNINative.kt.
        .kotlin_value_type("Sample", "io.zenoh.jni.Sample")
        .kotlin_value_type("MissDetectionConfig", "io.zenoh.jni.MissDetectionConfig")
        .kotlin_value_type("HistoryConfig", "io.zenoh.jni.HistoryConfig")
        .kotlin_value_type("CacheConfig", "io.zenoh.jni.CacheConfig")
        .kotlin_value_type("RecoveryConfig", "io.zenoh.jni.RecoveryConfig")
        .kotlin_value_type(
            "Option<MissDetectionConfig>",
            "io.zenoh.jni.MissDetectionConfig",
        )
        .kotlin_value_type("Option<HistoryConfig>", "io.zenoh.jni.HistoryConfig")
        .kotlin_value_type("Option<CacheConfig>", "io.zenoh.jni.CacheConfig")
        .kotlin_value_type("Option<RecoveryConfig>", "io.zenoh.jni.RecoveryConfig")
        // Callback arg types whose Kotlin form is the hand-written
        // JNIQueryableCallback / JNIGetCallback (the auto-emitted
        // JNIQueryCallback / JNIReplyCallback stubs are dead but must
        // compile, so we point them at `kotlin.Any`).
        .kotlin_value_type("Query", "kotlin.Any")
        .kotlin_value_type("Reply", "kotlin.Any")
        // ── impl Into<T> source arms.
        .into_sources(
            "ZKeyExpr<'static>",
            [
                IntoSource::borrow(syn::parse_quote!(ZKeyExpr<'static>)),
                IntoSource::borrow(syn::parse_quote!(String)),
            ],
        );

    // ── Write Rust bindings ───────────────────────────────────────────
    let source = prebindgen::Source::new(zenoh_flat::PREBINDGEN_OUT_DIR);
    let mut registry = Registry::from_items(source.items_all()).expect("scan failed");
    let rust_path = registry
        .write_rust(&jni, "zenoh_flat_jni.rs")
        .expect("write rust failed");
    println!(
        "cargo:warning=Generated bindings at: {}",
        rust_path.display()
    );

    // ── Write Kotlin output ───────────────────────────────────────────
    let kotlin_root = std::path::Path::new("../zenoh-jni-runtime/src/commonMain/kotlin");
    for path in jni
        .write_kotlin(&registry, kotlin_root)
        .expect("write kotlin failed")
    {
        println!("cargo:warning=Wrote {}", path.display());
    }
}
