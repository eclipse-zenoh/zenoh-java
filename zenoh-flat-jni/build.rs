use prebindgen::core::Registry;
use prebindgen::lang::JniGen;
use syn::parse_quote as pq;

fn fail(context: &str, err: impl std::fmt::Display) -> ! {
    eprintln!("error: prebindgen jnigen {context}: {err}");
    std::process::exit(1);
}

fn main() {
    // Flat tier only: every `#[prebindgen]` `z_*` function is declared as a
    // free function (`package_fun`) under its module namespace. Opaque handles
    // stay typed Kotlin classes derived from `NativeHandle` (locked, closeable)
    // via `ptr_class`, but functions are NOT represented as methods on them.
    // Errors are signalled through the per-call `ErrorSink` callback (no
    // Rust-side JVM exceptions); the generated wrappers install a default sink
    // that rethrows as `ZException`. `Error` is the `Result<_, Error>` error
    // type (a plain data class; no `.throwable()`). `ZZenohId` stays a
    // `value_blob` (`@JvmInline value class ZZenohId(val bytes: ByteArray)`).
    let jni = JniGen::new()
        .handle_locks(true)              // Enable handle locks (default, thread-safe)
        .source_module(pq!(zenoh_flat)) // how to prefix prebindgen-marked items (functions, types
        .package_prefix("io.zenoh.jni") // the package of the generated JNI bindings
        .data_class(pq!(Error)) // structured Kotlin data class for the Result error type
        .package("keyexpr")
        .ptr_class(pq!(ZKeyExpr))
        .package_fun(pq!(z_keyexpr_try_from))
        .package_fun(pq!(z_keyexpr_autocanonize))
        // Constructor expansion demo: a combined constructor for ZKeyExpr that
        // builds it either from a String (z_keyexpr_try_from) or accepts an
        // existing handle (identity). Expanding both params of
        // z_keyexpr_intersects lets Kotlin intersect a string key-expr with an
        // existing handle in a single JNI crossing instead of three.
        .combined_constructor(pq!(ZKeyExpr))
        .combined_variant(pq!(z_keyexpr_try_from))
        .combined_variant_id()
        .package_fun(pq!(z_keyexpr_intersects))
        .expand(pq!(a))
        .expand(pq!(b))
        .package_fun(pq!(z_keyexpr_includes))
        .package_fun(pq!(z_keyexpr_relation_to))
        .package_fun(pq!(z_keyexpr_join))
        .package_fun(pq!(z_keyexpr_concat))
        .package_fun(pq!(z_keyexpr_clone))
        .package_fun(pq!(z_keyexpr_to_string))
        .enum_class(pq!(SetIntersectionLevel))
        .package("config")
        .ptr_class(pq!(ZConfig))
        .package_fun(pq!(z_config_default))
        .package_fun(pq!(z_config_from_file))
        .package_fun(pq!(z_config_from_json))
        .package_fun(pq!(z_config_from_json5))
        .package_fun(pq!(z_config_from_yaml))
        .package_fun(pq!(z_config_get_json))
        .package_fun(pq!(z_config_insert_json5))
        .package_fun(pq!(z_config_clone))
        .enum_class(pq!(WhatAmI))
        // ZZenohId is a `Copy` value (zenoh::session::ZenohId, repr(transparent)),
        // so it crosses as a raw byte-blob `ByteArray` rather than a closeable
        // jlong handle — this also lets `Vec<ZZenohId>` surface as
        // `List<ZZenohId>` (see z_session_peers_zid/routers_zid below).
        .value_blob(pq!(ZZenohId))
        .package_fun(pq!(z_zenoh_id_to_bytes))
        .package_fun(pq!(z_zenoh_id_to_string))
        .package("scouting")
        .ptr_class(pq!(ZHello))
        .package_fun(pq!(z_hello_whatami))
        .package_fun(pq!(z_hello_zid))
        .package_fun(pq!(z_hello_locators))
        .ptr_class(pq!(ZScout))
        .package_fun(pq!(z_scout))
        .package("logger")
        .package_fun(pq!(init_android_logs))
        .package_fun(pq!(try_init_zenoh_logs_from_env))
        .package_fun(pq!(init_zenoh_logs_from_env_or))
        .package("qos")
        .enum_class(pq!(Reliability))
        .enum_class(pq!(Priority))
        .enum_class(pq!(CongestionControl))
        .package("bytes")
        .ptr_class(pq!(ZZBytes))
        .package_fun(pq!(z_zbytes_to_bytes))
        .package_fun(pq!(z_zbytes_clone))
        .package_fun(pq!(z_zbytes_from_vec))
        // NOTE: `z_zbytes_from_slice(&[u8])` is the C-pointer constructor shape
        // (`const uint8_t* + size`); its JNI form is `z_zbytes_from_vec(Vec<u8>)`
        // above (→ `ByteArray`). The `&[u8]` slice input has no JNI representation,
        // so it's intentionally not exported here.
        .ptr_class(pq!(ZEncoding))
        .package_fun(pq!(z_encoding_id))
        .package_fun(pq!(z_encoding_schema))
        .package_fun(pq!(z_encoding_to_string))
        .package_fun(pq!(z_encoding_clone))
        .package_fun(pq!(z_encoding_from_string))
        .package_fun(pq!(z_encoding_with_schema))
        .package_fun(pq!(z_encoding_zenoh_bytes))
        .package_fun(pq!(z_encoding_zenoh_string))
        .package_fun(pq!(z_encoding_zenoh_serialized))
        .package_fun(pq!(z_encoding_application_octet_stream))
        .package_fun(pq!(z_encoding_text_plain))
        .package_fun(pq!(z_encoding_application_json))
        .package_fun(pq!(z_encoding_text_json))
        .package_fun(pq!(z_encoding_application_cdr))
        .package_fun(pq!(z_encoding_application_cbor))
        .package_fun(pq!(z_encoding_application_yaml))
        .package_fun(pq!(z_encoding_text_yaml))
        .package_fun(pq!(z_encoding_text_json5))
        .package_fun(pq!(z_encoding_application_python_serialized_object))
        .package_fun(pq!(z_encoding_application_protobuf))
        .package_fun(pq!(z_encoding_application_java_serialized_object))
        .package_fun(pq!(z_encoding_application_openmetrics_text))
        .package_fun(pq!(z_encoding_image_png))
        .package_fun(pq!(z_encoding_image_jpeg))
        .package_fun(pq!(z_encoding_image_gif))
        .package_fun(pq!(z_encoding_image_bmp))
        .package_fun(pq!(z_encoding_image_webp))
        .package_fun(pq!(z_encoding_application_xml))
        .package_fun(pq!(z_encoding_application_x_www_form_urlencoded))
        .package_fun(pq!(z_encoding_text_html))
        .package_fun(pq!(z_encoding_text_xml))
        .package_fun(pq!(z_encoding_text_css))
        .package_fun(pq!(z_encoding_text_javascript))
        .package_fun(pq!(z_encoding_text_markdown))
        .package_fun(pq!(z_encoding_text_csv))
        .package_fun(pq!(z_encoding_application_sql))
        .package_fun(pq!(z_encoding_application_coap_payload))
        .package_fun(pq!(z_encoding_application_json_patch_json))
        .package_fun(pq!(z_encoding_application_json_seq))
        .package_fun(pq!(z_encoding_application_jsonpath))
        .package_fun(pq!(z_encoding_application_jwt))
        .package_fun(pq!(z_encoding_application_mp4))
        .package_fun(pq!(z_encoding_application_soap_xml))
        .package_fun(pq!(z_encoding_application_yang))
        .package_fun(pq!(z_encoding_audio_aac))
        .package_fun(pq!(z_encoding_audio_flac))
        .package_fun(pq!(z_encoding_audio_mp4))
        .package_fun(pq!(z_encoding_audio_ogg))
        .package_fun(pq!(z_encoding_audio_vorbis))
        .package_fun(pq!(z_encoding_video_h261))
        .package_fun(pq!(z_encoding_video_h263))
        .package_fun(pq!(z_encoding_video_h264))
        .package_fun(pq!(z_encoding_video_h265))
        .package_fun(pq!(z_encoding_video_h266))
        .package_fun(pq!(z_encoding_video_mp4))
        .package_fun(pq!(z_encoding_video_ogg))
        .package_fun(pq!(z_encoding_video_raw))
        .package_fun(pq!(z_encoding_video_vp8))
        .package_fun(pq!(z_encoding_video_vp9))
        .package("time")
        .ptr_class(pq!(ZTimestamp))
        .package_fun(pq!(z_timestamp_ntp64))
        .package_fun(pq!(z_timestamp_id))
        .package("sample")
        .enum_class(pq!(SampleKind))
        .ptr_class(pq!(ZSample))
        .package_fun(pq!(z_sample_key_expr))
        .package_fun(pq!(z_sample_payload))
        .package_fun(pq!(z_sample_encoding))
        .package_fun(pq!(z_sample_kind))
        .package_fun(pq!(z_sample_timestamp))
        .package_fun(pq!(z_sample_express))
        .package_fun(pq!(z_sample_priority))
        .package_fun(pq!(z_sample_congestion_control))
        .package_fun(pq!(z_sample_attachment))
        .package("pubsub")
        .ptr_class(pq!(ZPublisher))
        .package_fun(pq!(z_publisher_put))
        .package_fun(pq!(z_publisher_delete))
        .ptr_class(pq!(ZSubscriber))
        .package("query")
        .ptr_class(pq!(ZQueryable))
        .ptr_class(pq!(ZQuerier))
        .package_fun(pq!(z_querier_get))
        .enum_class(pq!(ReplyKeyExpr))
        .enum_class(pq!(QueryTarget))
        .enum_class(pq!(ConsolidationMode))
        .ptr_class(pq!(ZQuery))
        .package_fun(pq!(z_query_reply_success))
        .package_fun(pq!(z_query_reply_error))
        .package_fun(pq!(z_query_reply_delete))
        .package_fun(pq!(z_query_keyexpr))
        .package_fun(pq!(z_query_parameters))
        .package_fun(pq!(z_query_payload))
        .package_fun(pq!(z_query_encoding))
        .package_fun(pq!(z_query_attachment))
        .package_fun(pq!(z_query_accepts_replies))
        .ptr_class(pq!(ZReply))
        .package_fun(pq!(z_reply_replier_zid))
        .package_fun(pq!(z_reply_replier_eid))
        .package_fun(pq!(z_reply_is_ok))
        .package_fun(pq!(z_reply_sample))
        .package_fun(pq!(z_reply_error_payload))
        .package_fun(pq!(z_reply_error_encoding))
        .package("liveliness")
        .ptr_class(pq!(ZLivelinessToken))
        .package("session")
        .ptr_class(pq!(ZSession))
        .package_fun(pq!(z_open))
        .package_fun(pq!(z_session_declare_publisher))
        .package_fun(pq!(z_session_put))
        .package_fun(pq!(z_session_delete))
        .package_fun(pq!(z_session_declare_subscriber))
        .package_fun(pq!(z_session_declare_querier))
        .package_fun(pq!(z_session_declare_queryable))
        .package_fun(pq!(z_session_declare_keyexpr))
        .package_fun(pq!(z_session_undeclare_keyexpr))
        .package_fun(pq!(z_session_get))
        .package_fun(pq!(z_session_zid))
        .package_fun(pq!(z_session_peers_zid))
        .package_fun(pq!(z_session_routers_zid))
        .package_fun(pq!(z_liveliness_declare_token))
        .package_fun(pq!(z_liveliness_get))
        .package_fun(pq!(z_liveliness_declare_subscriber));

    let source = prebindgen::Source::new(zenoh_flat::PREBINDGEN_OUT_DIR);
    let mut registry = match Registry::from_items(source.items_all()) {
        Ok(registry) => registry,
        Err(err) => fail("scan failed", err),
    };
    let rust_path = match registry.write_rust(&jni, "zenoh_flat_jni.rs") {
        Ok(path) => path,
        Err(err) => fail("write_rust failed", err),
    };
    println!(
        "cargo:warning=Generated bindings at: {}",
        rust_path.display()
    );

    // ── Write Kotlin output ───────────────────────────────────────────
    // All generated Kotlin lives under `generated-kotlin/`; the runtime
    // module's Gradle source set picks it up via
    // `kotlin.srcDir("$rootDir/zenoh-flat-jni/generated-kotlin")`.
    let kotlin_root = std::path::Path::new("generated-kotlin");
    // Remove stale generated files so package moves don't leave old classes
    // behind (e.g. io/zenoh/jni/* and io/zenoh/jni/<subpackage>/* side-by-side).
    if let Err(err) = std::fs::remove_dir_all(kotlin_root) {
        if err.kind() != std::io::ErrorKind::NotFound {
            fail("cleanup generated-kotlin failed", err);
        }
    }
    for path in match jni.write_kotlin(&registry, kotlin_root) {
        Ok(paths) => paths,
        Err(err) => fail("write_kotlin failed", err),
    } {
        println!("cargo:warning=Wrote {}", path.display());
    }
}
