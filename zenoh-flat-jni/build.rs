use prebindgen::core::{IntoSource, Registry};
use prebindgen::lang::JniGen;
use syn::parse_quote as pq;

fn fail(context: &str, err: impl std::fmt::Display) -> ! {
    eprintln!("error: prebindgen jnigen {context}: {err}");
    std::process::exit(1);
}

fn main() {
    let jni = JniGen::new()
        .source_module(pq!(zenoh_flat)) // how to prefix prebindgen-marked items (functions, types
        .package_prefix("io.zenoh.jni") // the package of the generated JNI bindings
        .data_class(pq!(Error)) // structured Kotlin data class for Error
        .throwable()                    // …also throwable; JniExt's built-in
                                        // rank-2 Result<_, _> wrapper routes
                                        // Err(Error) through it on the JVM side.
        .package("keyexpr")
        .ptr_class(pq!(ZKeyExpr))
        .class_object_fun(pq!(z_keyexpr_try_from))
        .class_object_fun(pq!(z_keyexpr_autocanonize))
        .class_fun(pq!(z_keyexpr_intersects))
        .class_fun(pq!(z_keyexpr_includes))
        .class_fun(pq!(z_keyexpr_relation_to))
        .class_fun(pq!(z_keyexpr_join))
        .class_fun(pq!(z_keyexpr_concat))
        .enum_class(pq!(SetIntersectionLevel))
        .data_class(pq!(KeyExpr))
        .class_object_fun(pq!(keyexpr_try_from))
        .class_object_fun(pq!(keyexpr_autocanonize))
        .class_object_fun(pq!(keyexpr_intersects))
        .class_object_fun(pq!(keyexpr_includes))
        .class_object_fun(pq!(keyexpr_relation_to))
        .class_object_fun(pq!(keyexpr_join))
        .class_object_fun(pq!(keyexpr_concat))
        .into_sources(
            pq!(KeyExpr),
            [
                IntoSource::borrow(pq!(KeyExpr)),
                IntoSource::borrow(pq!(ZKeyExpr)),
                IntoSource::borrow(pq!(String)),
            ],
        )
        .package("config")
        .ptr_class(pq!(ZConfig))
        .class_object_fun(pq!(z_config_default))
        .class_object_fun(pq!(z_config_from_file))
        .class_object_fun(pq!(z_config_from_json))
        .class_object_fun(pq!(z_config_from_json5))
        .class_object_fun(pq!(z_config_from_yaml))
        .class_fun(pq!(z_config_get_json))
        .class_fun(pq!(z_config_insert_json5))
        .enum_class(pq!(WhatAmI))
        // ZZenohId is a `Copy` value (zenoh::session::ZenohId, repr(transparent)),
        // so it crosses as a raw byte-blob `ByteArray` rather than a closeable
        // jlong handle — this also lets `Vec<ZZenohId>` surface as
        // `List<ByteArray>` (see z_session_peers_zid/routers_zid below). Its
        // accessors become package-level functions (no Kotlin class for a blob).
        .value_blob(pq!(ZZenohId))
        .package_fun(pq!(z_zenoh_id_to_bytes))
        .package_fun(pq!(z_zenoh_id_to_string))
        .value_class(pq!(ZenohId))
        .class_object_fun(pq!(zenoh_id_to_string))
        .package("scouting")
        .ptr_class(pq!(ZHello))
        .class_fun(pq!(z_hello_whatami))
        .class_fun(pq!(z_hello_zid))
        .class_fun(pq!(z_hello_locators))
        .data_class(pq!(Hello))
        .ptr_class(pq!(ZScout))
        .package_fun(pq!(z_scout))
        .package_fun(pq!(scout))
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
        .class_fun(pq!(z_zbytes_to_bytes))
        .class_object_fun(pq!(z_zbytes_from_vec))
        .value_class(pq!(ZBytes))
        .ptr_class(pq!(ZEncoding))
        .class_fun(pq!(z_encoding_id))
        .class_fun(pq!(z_encoding_schema))
        .class_fun(pq!(z_encoding_to_string))
        .class_object_fun(pq!(z_encoding_from_string))
        .class_object_fun(pq!(z_encoding_zenoh_bytes))
        .class_object_fun(pq!(z_encoding_zenoh_string))
        .class_object_fun(pq!(z_encoding_zenoh_serialized))
        .class_object_fun(pq!(z_encoding_application_octet_stream))
        .class_object_fun(pq!(z_encoding_text_plain))
        .class_object_fun(pq!(z_encoding_application_json))
        .class_object_fun(pq!(z_encoding_text_json))
        .class_object_fun(pq!(z_encoding_application_cdr))
        .class_object_fun(pq!(z_encoding_application_cbor))
        .class_object_fun(pq!(z_encoding_application_yaml))
        .class_object_fun(pq!(z_encoding_text_yaml))
        .class_object_fun(pq!(z_encoding_text_json5))
        .class_object_fun(pq!(z_encoding_application_python_serialized_object))
        .class_object_fun(pq!(z_encoding_application_protobuf))
        .class_object_fun(pq!(z_encoding_application_java_serialized_object))
        .class_object_fun(pq!(z_encoding_application_openmetrics_text))
        .class_object_fun(pq!(z_encoding_image_png))
        .class_object_fun(pq!(z_encoding_image_jpeg))
        .class_object_fun(pq!(z_encoding_image_gif))
        .class_object_fun(pq!(z_encoding_image_bmp))
        .class_object_fun(pq!(z_encoding_image_webp))
        .class_object_fun(pq!(z_encoding_application_xml))
        .class_object_fun(pq!(z_encoding_application_x_www_form_urlencoded))
        .class_object_fun(pq!(z_encoding_text_html))
        .class_object_fun(pq!(z_encoding_text_xml))
        .class_object_fun(pq!(z_encoding_text_css))
        .class_object_fun(pq!(z_encoding_text_javascript))
        .class_object_fun(pq!(z_encoding_text_markdown))
        .class_object_fun(pq!(z_encoding_text_csv))
        .class_object_fun(pq!(z_encoding_application_sql))
        .class_object_fun(pq!(z_encoding_application_coap_payload))
        .class_object_fun(pq!(z_encoding_application_json_patch_json))
        .class_object_fun(pq!(z_encoding_application_json_seq))
        .class_object_fun(pq!(z_encoding_application_jsonpath))
        .class_object_fun(pq!(z_encoding_application_jwt))
        .class_object_fun(pq!(z_encoding_application_mp4))
        .class_object_fun(pq!(z_encoding_application_soap_xml))
        .class_object_fun(pq!(z_encoding_application_yang))
        .class_object_fun(pq!(z_encoding_audio_aac))
        .class_object_fun(pq!(z_encoding_audio_flac))
        .class_object_fun(pq!(z_encoding_audio_mp4))
        .class_object_fun(pq!(z_encoding_audio_ogg))
        .class_object_fun(pq!(z_encoding_audio_vorbis))
        .class_object_fun(pq!(z_encoding_video_h261))
        .class_object_fun(pq!(z_encoding_video_h263))
        .class_object_fun(pq!(z_encoding_video_h264))
        .class_object_fun(pq!(z_encoding_video_h265))
        .class_object_fun(pq!(z_encoding_video_h266))
        .class_object_fun(pq!(z_encoding_video_mp4))
        .class_object_fun(pq!(z_encoding_video_ogg))
        .class_object_fun(pq!(z_encoding_video_raw))
        .class_object_fun(pq!(z_encoding_video_vp8))
        .class_object_fun(pq!(z_encoding_video_vp9))
        .data_class(pq!(Encoding))
        .class_fun(pq!(encoding_to_string))
        .class_object_fun(pq!(encoding_from_string))
        .class_object_fun(pq!(encoding_zenoh_bytes))
        .class_object_fun(pq!(encoding_zenoh_string))
        .class_object_fun(pq!(encoding_zenoh_serialized))
        .class_object_fun(pq!(encoding_application_octet_stream))
        .class_object_fun(pq!(encoding_text_plain))
        .class_object_fun(pq!(encoding_application_json))
        .class_object_fun(pq!(encoding_text_json))
        .class_object_fun(pq!(encoding_application_cdr))
        .class_object_fun(pq!(encoding_application_cbor))
        .class_object_fun(pq!(encoding_application_yaml))
        .class_object_fun(pq!(encoding_text_yaml))
        .class_object_fun(pq!(encoding_text_json5))
        .class_object_fun(pq!(encoding_application_python_serialized_object))
        .class_object_fun(pq!(encoding_application_protobuf))
        .class_object_fun(pq!(encoding_application_java_serialized_object))
        .class_object_fun(pq!(encoding_application_openmetrics_text))
        .class_object_fun(pq!(encoding_image_png))
        .class_object_fun(pq!(encoding_image_jpeg))
        .class_object_fun(pq!(encoding_image_gif))
        .class_object_fun(pq!(encoding_image_bmp))
        .class_object_fun(pq!(encoding_image_webp))
        .class_object_fun(pq!(encoding_application_xml))
        .class_object_fun(pq!(encoding_application_x_www_form_urlencoded))
        .class_object_fun(pq!(encoding_text_html))
        .class_object_fun(pq!(encoding_text_xml))
        .class_object_fun(pq!(encoding_text_css))
        .class_object_fun(pq!(encoding_text_javascript))
        .class_object_fun(pq!(encoding_text_markdown))
        .class_object_fun(pq!(encoding_text_csv))
        .class_object_fun(pq!(encoding_application_sql))
        .class_object_fun(pq!(encoding_application_coap_payload))
        .class_object_fun(pq!(encoding_application_json_patch_json))
        .class_object_fun(pq!(encoding_application_json_seq))
        .class_object_fun(pq!(encoding_application_jsonpath))
        .class_object_fun(pq!(encoding_application_jwt))
        .class_object_fun(pq!(encoding_application_mp4))
        .class_object_fun(pq!(encoding_application_soap_xml))
        .class_object_fun(pq!(encoding_application_yang))
        .class_object_fun(pq!(encoding_audio_aac))
        .class_object_fun(pq!(encoding_audio_flac))
        .class_object_fun(pq!(encoding_audio_mp4))
        .class_object_fun(pq!(encoding_audio_ogg))
        .class_object_fun(pq!(encoding_audio_vorbis))
        .class_object_fun(pq!(encoding_video_h261))
        .class_object_fun(pq!(encoding_video_h263))
        .class_object_fun(pq!(encoding_video_h264))
        .class_object_fun(pq!(encoding_video_h265))
        .class_object_fun(pq!(encoding_video_h266))
        .class_object_fun(pq!(encoding_video_mp4))
        .class_object_fun(pq!(encoding_video_ogg))
        .class_object_fun(pq!(encoding_video_raw))
        .class_object_fun(pq!(encoding_video_vp8))
        .class_object_fun(pq!(encoding_video_vp9))
        .package("time")
        .ptr_class(pq!(ZTimestamp))
        .class_fun(pq!(z_timestamp_ntp64))
        .class_fun(pq!(z_timestamp_id))
        .class_fun(pq!(z_timestamp_expand))
        .data_class(pq!(Timestamp))
        .package("sample")
        .enum_class(pq!(SampleKind))
        .ptr_class(pq!(ZSample))
        .class_fun(pq!(z_sample_key_expr))
        .class_fun(pq!(z_sample_payload))
        .class_fun(pq!(z_sample_encoding))
        .class_fun(pq!(z_sample_kind))
        .class_fun(pq!(z_sample_timestamp))
        .class_fun(pq!(z_sample_express))
        .class_fun(pq!(z_sample_priority))
        .class_fun(pq!(z_sample_congestion_control))
        .class_fun(pq!(z_sample_attachment))
        .class_fun(pq!(z_sample_expand))
        .data_class(pq!(Sample))
        .package("pubsub")
        .ptr_class(pq!(ZPublisher))
        .class_fun(pq!(z_publisher_put))
        .class_fun(pq!(z_publisher_delete))
        .ptr_class(pq!(ZSubscriber))
        .package("query")
        .ptr_class(pq!(ZQueryable))
        .ptr_class(pq!(ZQuerier))
        .class_fun(pq!(z_querier_get))
        .class_fun(pq!(querier_get))
        .enum_class(pq!(ReplyKeyExpr))
        .enum_class(pq!(QueryTarget))
        .enum_class(pq!(ConsolidationMode))
        .ptr_class(pq!(ZQuery))
        .class_fun(pq!(z_query_reply_success))
        .class_fun(pq!(z_query_reply_error))
        .class_fun(pq!(z_query_reply_delete))
        .class_fun(pq!(z_query_expand))
        .data_class(pq!(Query))
        .ptr_class(pq!(ZReply))
        .class_fun(pq!(z_reply_replier_zid))
        .class_fun(pq!(z_reply_replier_eid))
        .class_fun(pq!(z_reply_is_ok))
        .class_fun(pq!(z_reply_sample))
        .class_fun(pq!(z_reply_error_payload))
        .class_fun(pq!(z_reply_error_encoding))
        .class_fun(pq!(z_reply_expand))
        .data_class(pq!(Reply))
        .package("liveliness")
        .ptr_class(pq!(ZLivelinessToken))
        .package("session")
        .ptr_class(pq!(ZSession))
        .class_object_fun(pq!(z_open))
        .class_fun(pq!(z_session_declare_publisher))
        .class_fun(pq!(z_session_put))
        .class_fun(pq!(z_session_delete))
        .class_fun(pq!(z_session_declare_subscriber))
        .class_fun(pq!(session_declare_subscriber))
        .class_fun(pq!(z_session_declare_querier))
        .class_fun(pq!(z_session_declare_queryable))
        .class_fun(pq!(session_declare_queryable))
        .class_fun(pq!(z_session_declare_keyexpr))
        .class_fun(pq!(z_session_undeclare_keyexpr))
        .class_fun(pq!(z_session_get))
        .class_fun(pq!(session_get))
        .class_fun(pq!(z_session_zid))
        // `Vec<ZZenohId>` → `List<ByteArray>` now that ZZenohId is a value-blob.
        .class_fun(pq!(z_session_peers_zid))
        .class_fun(pq!(z_session_routers_zid))
        .class_fun(pq!(z_liveliness_declare_token))
        .class_fun(pq!(z_liveliness_get))
        .class_fun(pq!(liveliness_get))
        .class_fun(pq!(z_liveliness_declare_subscriber))
        .class_fun(pq!(liveliness_declare_subscriber))
        .into_sources(
            pq!(ZBytes),
            [
                IntoSource::borrow(pq!(ZZBytes)),
                IntoSource::borrow(pq!(ZBytes)),
                IntoSource::borrow(pq!(Vec<u8>)),
            ],
        )
        .into_sources(
            pq!(Encoding),
            [
                IntoSource::borrow(pq!(Encoding)),
                IntoSource::borrow(pq!(ZEncoding)),
                IntoSource::borrow(pq!(String)),
            ],
        )
        ;

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
