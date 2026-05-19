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
        // ── Kotlin classes — `kotlin_class` configures: jlong wire
        // (input + output), `Box::into_raw`/`Box::from_raw` lifecycle,
        // `instanceof` dispatch class, and the Kotlin parameter-type
        // name. A typed-handle Kotlin shell class is auto-emitted by
        // default; chain `.method(...)` (repeat for each promoted
        // instance method) to add methods, or `.suppress_kotlin_code()`
        // to opt out of emission when the `.kt` file is hand-written.
        .kotlin_class("Session", "io.zenoh.jni.JNISession")
        .suppress_kotlin_code()
        .kotlin_class("Config", "io.zenoh.jni.JNIConfig")
        .suppress_kotlin_code()
        .kotlin_class("ZKeyExpr<'static>", "io.zenoh.jni.JNIKeyExpr")
        .suppress_kotlin_code()
        .kotlin_class("Publisher<'static>", "io.zenoh.jni.JNIPublisher")
        .method("put_publisher")
        .method("delete_publisher")
        .kotlin_class("Subscriber<()>", "io.zenoh.jni.JNISubscriber")
        .kotlin_class("Querier<'static>", "io.zenoh.jni.JNIQuerier")
        .suppress_kotlin_code()
        .kotlin_class("Queryable<()>", "io.zenoh.jni.JNIQueryable")
        .kotlin_class(
            "AdvancedSubscriber<()>",
            "io.zenoh.jni.JNIAdvancedSubscriber",
        )
        .suppress_kotlin_code()
        .kotlin_class(
            "AdvancedPublisher<'static>",
            "io.zenoh.jni.JNIAdvancedPublisher",
        )
        .suppress_kotlin_code()
        .kotlin_class("MatchingListener", "io.zenoh.jni.JNIMatchingListener")
        .kotlin_class("SampleMissListener", "io.zenoh.jni.JNISampleMissListener")
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
        // ── Value-shaped custom converters. Non-primitive wires
        // (`JObject` / `jobject`) chain `with_kotlin_name` to bind the
        // value-context Kotlin type; primitive wires (`jint`,
        // `jbyteArray`) auto-derive via `kotlin_for_wire`.
        .input_decoder(
            "Encoding",
            "jni::objects::JObject",
            "crate::utils::decode_jni_encoding(env, &v)?",
        )
        .with_kotlin_name("io.zenoh.jni.JNIEncoding")
        .input_decoder(
            "Option<Encoding>",
            "jni::objects::JObject",
            "if !v.is_null() { Some(crate::utils::decode_jni_encoding(env, &v)?) } else { None }",
        )
        .with_kotlin_name("io.zenoh.jni.JNIEncoding")
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
        .with_kotlin_name("List<ByteArray>")
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
        // ── Kotlin names for hand-maintained data classes whose
        // converters auto-generate from the struct shape but whose
        // Kotlin form lives in JNINative.kt. Primitives, opaque
        // handles, `jint_enum`s, `with_kotlin_name`-tagged
        // decoders/encoders, and every `Option<T>` / `ZResult<T>` /
        // `&T` wrapper auto-derive their Kotlin names through the
        // [`KotlinMeta`] propagation in the rank-N handlers — no
        // build.rs entry needed.
        .kotlin_value_type("Sample", "io.zenoh.jni.Sample")
        .kotlin_value_type("MissDetectionConfig", "io.zenoh.jni.MissDetectionConfig")
        .kotlin_value_type("HistoryConfig", "io.zenoh.jni.HistoryConfig")
        .kotlin_value_type("CacheConfig", "io.zenoh.jni.CacheConfig")
        .kotlin_value_type("RecoveryConfig", "io.zenoh.jni.RecoveryConfig")
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
