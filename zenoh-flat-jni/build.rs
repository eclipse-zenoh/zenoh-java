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
        // Combined constructor for ZKeyExpr: a key-expr parameter accepts EITHER
        // a String (built via z_keyexpr_try_from) OR an existing declared handle
        // (identity). This is semantic, not just perf — a declared key-expr is a
        // network-optimized resource distinct from a raw string. Every key-expr
        // consumer below is `.expand`ed so callers pass a string or a handle in
        // one JNI crossing. (Exception: z_session_undeclare_keyexpr stays
        // handle-only — see the session package.)
        .constructor(pq!(ZKeyExpr))
        .constructor_variant(pq!(z_keyexpr_try_from))
        .constructor_variant_id()
        // Combined ACCESSOR for ZKeyExpr (output expansion): a function
        // returning a key-expr handle (`.deconstruct_output()`) is decomposed into
        // BOTH the handle (identity record) and its borrowed string form
        // (`z_keyexpr_as_str`, a zero-copy `&str → jstring`), delivered to a
        // zenoh-java builder lambda in one JNI crossing. zenoh-java builds its
        // `KeyExpr(flat, string)` directly and later sends the handle back (its
        // `exprSel` selects the identity arm of the combined constructor above).
        .deconstructor(pq!(ZKeyExpr))
        .deconstructor_record_id()
        .deconstructor_record(pq!(z_keyexpr_as_str))
        .package_fun(pq!(z_keyexpr_intersects))
        .construct(pq!(a))
        .construct(pq!(b))
        .package_fun(pq!(z_keyexpr_includes))
        .construct(pq!(a))
        .construct(pq!(b))
        .package_fun(pq!(z_keyexpr_relation_to))
        .construct(pq!(a))
        .construct(pq!(b))
        .package_fun(pq!(z_keyexpr_join)) // b is a String, only `a` is a key-expr
        .construct(pq!(a))
        .package_fun(pq!(z_keyexpr_concat)) // b is a String, only `a` is a key-expr
        .construct(pq!(a))
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
        // jlong handle. `Vec<ZZenohId>` (z_session_peers_zid/routers_zid) folds
        // each element WHOLE as the typed `ZZenohId` value class (Iterable,
        // no accessor). NOTE: the vector-of-*unfolded* machinery (decompose each
        // element into e.g. `(String, ZZenohId)` via an `.deconstructor(ZZenohId)`
        // with `.deconstructor_record_id()` — a `value_blob` identity delivered by
        // copy) is implemented and unit-tested (`iterable_decomposed_plan`); it's
        // simply not wired here because the SDK `ZenohId` stores the blob only
        // and computes its string lazily.
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
        // Constructor for ZZBytes: payload/attachment params accept a
        // `ByteArray` (built via z_zbytes_from_vec) directly — no handle, no
        // per-call `zZbytesFromVec` crossing. (One variant, no identity arm: the
        // SDK never holds a ZZBytes handle for these.)
        .constructor(pq!(ZZBytes))
        .constructor_variant(pq!(z_zbytes_from_vec))
        // CONVERTER for ZZBytes (single-value deconstructor): an `Option<&ZZBytes>`
        // return (z_sample_attachment) is converted to its bytes
        // (`z_zbytes_to_bytes` → ByteArray) and **returned** directly via
        // `.convert_output()` (no callback). Also doubles as the nested record
        // for the ZSample deconstructor's payload.
        .converter(pq!(ZZBytes), pq!(z_zbytes_to_bytes))
        .ptr_class(pq!(ZEncoding))
        .package_fun(pq!(z_encoding_id))
        .package_fun(pq!(z_encoding_schema))
        .package_fun(pq!(z_encoding_to_string))
        .package_fun(pq!(z_encoding_clone))
        .package_fun(pq!(z_encoding_from_string))
        // CONVERTER for ZEncoding: decompose to its canonical string
        // (`z_encoding_to_string`), nested by the ZSample deconstructor below to
        // build the SDK `Encoding(string)`.
        .converter(pq!(ZEncoding), pq!(z_encoding_to_string))
        // Constructor for ZEncoding: encoding params accept a `String`
        // (built via z_encoding_from_string) directly — the SDK passes its
        // canonical `repr` String, no per-call `zEncodingFromString` + close.
        .constructor(pq!(ZEncoding))
        .constructor_variant(pq!(z_encoding_from_string))
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
        // CONVERTER for ZTimestamp (single-value): an `Option<&ZTimestamp>`
        // return (z_sample_timestamp) is converted to its NTP64 value
        // (`z_timestamp_ntp64` → i64) and **returned** as `Long?` via
        // `.convert_output()` — no callback, fewer JNI crossings than a builder.
        // Also the nested record for ZSample's timestamp.
        .converter(pq!(ZTimestamp), pq!(z_timestamp_ntp64))
        .package("sample")
        .enum_class(pq!(SampleKind))
        .ptr_class(pq!(ZSample))
        .package_fun(pq!(z_sample_key_expr))
        .deconstruct_output() // &ZKeyExpr → builder (ZKeyExpr handle, String)
        .package_fun(pq!(z_sample_payload))
        .package_fun(pq!(z_sample_encoding))
        .package_fun(pq!(z_sample_kind))
        .package_fun(pq!(z_sample_timestamp))
        .convert_output() // Option<&ZTimestamp> → returns Long? (no callback)
        .package_fun(pq!(z_sample_express))
        .package_fun(pq!(z_sample_priority))
        .package_fun(pq!(z_sample_congestion_control))
        .package_fun(pq!(z_sample_attachment))
        .convert_output() // Option<&ZZBytes> → returns ByteArray? (no callback)
        // Combined ACCESSOR for ZSample (output expansion, M3): the full sample
        // decomposed in ONE crossing — NESTS the ZKeyExpr (handle+string),
        // ZZBytes (payload bytes), ZEncoding (string) and ZTimestamp (ntp64,
        // nullable) combined accessors, plus enum leaves (kind/priority/
        // congestion → Int) and `express` (bool). Record order = builder arg
        // order. Used by `z_reply_sample` below to build a full SDK `Sample`.
        .deconstructor(pq!(ZSample))
        .deconstructor_record_nested(pq!(z_sample_key_expr)) // → (ZKeyExpr, String)
        .deconstructor_record_nested(pq!(z_sample_payload)) // → ByteArray
        .deconstructor_record_nested(pq!(z_sample_encoding)) // → String
        .deconstructor_record(pq!(z_sample_kind)) // enum → Int
        .deconstructor_record_nested(pq!(z_sample_timestamp)) // Option → Long?
        .deconstructor_record(pq!(z_sample_express)) // bool → Boolean
        .deconstructor_record(pq!(z_sample_priority)) // enum → Int
        .deconstructor_record(pq!(z_sample_congestion_control)) // enum → Int
        .deconstructor_record_nested(pq!(z_sample_attachment)) // Option → ByteArray?
        .package("pubsub")
        .ptr_class(pq!(ZPublisher))
        .package_fun(pq!(z_publisher_put))
        .construct(pq!(payload)) // ZZBytes ← ByteArray
        .construct(pq!(encoding)) // Option<&ZEncoding> ← String?
        .construct(pq!(attachment)) // Option<ZZBytes> ← ByteArray?
        .package_fun(pq!(z_publisher_delete))
        .construct(pq!(attachment))
        .ptr_class(pq!(ZSubscriber))
        .package("query")
        .ptr_class(pq!(ZQueryable))
        .ptr_class(pq!(ZQuerier))
        .package_fun(pq!(z_querier_get))
        .construct(pq!(payload)) // Option<ZZBytes> ← ByteArray?
        .construct(pq!(encoding)) // Option<&ZEncoding> ← String?
        .construct(pq!(attachment)) // Option<ZZBytes> ← ByteArray?
        .enum_class(pq!(ReplyKeyExpr))
        .enum_class(pq!(QueryTarget))
        .enum_class(pq!(ConsolidationMode))
        .ptr_class(pq!(ZQuery))
        .package_fun(pq!(z_query_reply_success))
        .construct(pq!(key_expr)) // &ZKeyExpr ← String | handle
        .construct(pq!(payload)) // ZZBytes ← ByteArray
        .construct(pq!(encoding)) // Option<&ZEncoding> ← String?
        .construct(pq!(attachment)) // Option<ZZBytes> ← ByteArray?
        .package_fun(pq!(z_query_reply_error))
        .construct(pq!(payload)) // ZZBytes ← ByteArray
        .construct(pq!(encoding)) // Option<&ZEncoding> ← String?
        .package_fun(pq!(z_query_reply_delete))
        .construct(pq!(key_expr)) // &ZKeyExpr ← String | handle
        .construct(pq!(attachment)) // Option<ZZBytes> ← ByteArray?
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
        .deconstruct_output() // Option<&ZSample> → builder(full Sample leaves) → R?
        .package_fun(pq!(z_reply_error_payload))
        .package_fun(pq!(z_reply_error_encoding))
        .package("liveliness")
        .ptr_class(pq!(ZLivelinessToken))
        .package("session")
        .ptr_class(pq!(ZSession))
        .package_fun(pq!(z_open))
        .package_fun(pq!(z_session_declare_publisher))
        .construct(pq!(key_expr)) // ZKeyExpr (by-value) ← String | handle
        .package_fun(pq!(z_session_put))
        .construct(pq!(key_expr)) // &ZKeyExpr ← String | handle
        .construct(pq!(payload)) // ZZBytes ← ByteArray
        .construct(pq!(encoding)) // Option<&ZEncoding> ← String?
        .construct(pq!(attachment)) // Option<ZZBytes> ← ByteArray?
        .package_fun(pq!(z_session_delete))
        .construct(pq!(key_expr)) // &ZKeyExpr ← String | handle
        .construct(pq!(attachment)) // Option<ZZBytes> ← ByteArray?
        .package_fun(pq!(z_session_declare_subscriber))
        .construct(pq!(key_expr)) // ZKeyExpr (by-value) ← String | handle
        .package_fun(pq!(z_session_declare_querier))
        .construct(pq!(key_expr))
        .package_fun(pq!(z_session_declare_queryable))
        .construct(pq!(key_expr))
        .package_fun(pq!(z_session_declare_keyexpr))
        // z_session_undeclare_keyexpr: NOT expanded — undeclaring requires a
        // declared handle, not a string. Stays handle-only.
        .package_fun(pq!(z_session_undeclare_keyexpr))
        .package_fun(pq!(z_session_get))
        .construct(pq!(key_expr)) // &ZKeyExpr ← String | handle
        .construct(pq!(payload)) // Option<ZZBytes> ← ByteArray?
        .construct(pq!(encoding)) // Option<&ZEncoding> ← String?
        .construct(pq!(attachment)) // Option<ZZBytes> ← ByteArray?
        .package_fun(pq!(z_session_zid))
        // Output expansion (M4, Iterable): Vec<ZZenohId> → fold, each ZZenohId
        // delivered WHOLE (its value_blob projection); caller owns the result
        // collection. No combined accessor — the element crosses as the typed
        // `ZZenohId` value class, matching the prior `List<ZZenohId>`.
        .package_fun(pq!(z_session_peers_zid))
        .deconstruct_output() // Vec<ZZenohId> → fun <A>(acc, fold: (A, ZZenohId) -> A): A
        .package_fun(pq!(z_session_routers_zid))
        .deconstruct_output()
        .package_fun(pq!(z_liveliness_declare_token))
        .construct(pq!(key_expr)) // ZKeyExpr (by-value) ← String | handle
        .package_fun(pq!(z_liveliness_get))
        .construct(pq!(key_expr)) // &ZKeyExpr ← String | handle
        .package_fun(pq!(z_liveliness_declare_subscriber))
        .construct(pq!(key_expr)); // ZKeyExpr (by-value) ← String | handle

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
