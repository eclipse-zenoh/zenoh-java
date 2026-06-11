use prebindgen::core::Registry;
use prebindgen::lang::JniGen;
use syn::parse_quote as pq;

fn fail(context: &str, err: impl std::fmt::Display) -> ! {
    eprintln!("error: prebindgen jnigen {context}: {err}");
    std::process::exit(1);
}

fn main() {
    // Flat tier only: every `#[prebindgen]` `z_*` function is declared as a
    // free function (`.fun` / `.fun_accessor`) under its module namespace. Opaque
    // handles stay typed Kotlin classes derived from `NativeHandle` (locked,
    // closeable) via `ptr_class`. A type's CANONICAL representation — its input
    // (`.ptr_class_input*`) and output (`.ptr_class_output*`) — is declared on the
    // `ptr_class` and AUTO-APPLIES to every matching param / return; `.fun_accessor`
    // fns and per-fn `.fun_input_direct`/`.fun_output_direct` opt out. Errors are
    // delivered through the per-call `onError` callback (no Rust-side JVM throw).
    // `ZZenohId` stays a `value_blob` (`@JvmInline value class`).
    let jni = JniGen::new()
        .handle_locks(true) // Enable handle locks (default, thread-safe)
        .source_module(pq!(zenoh_flat)) // how to prefix prebindgen-marked items (functions, types
        .package_prefix("io.zenoh.jni") // the package of the generated JNI bindings
        // Error model: zenoh's native `ZError` is the `E` of every fallible
        // `Result<_, ZError>`. Its canonical output (`z_error_message -> String`,
        // declared below) auto-applies to the `E` position of every such Result,
        // so the wrapper's `onError` callback receives the error string as its
        // single `ze` param (after the fixed binding `je: String?`). No throw from
        // zenoh-flat-jni; the caller's `onError` decides (zenoh-java throws `ZError`).
        //
        // `ptr_class(ZError)` makes `ZError` a regular opaque-handle type so the
        // rank-2 `Result<T, ZError>` resolver finds its input/output converters
        // (jlong handle). The handle never actually crosses — the extern peels the
        // `Result` and delivers the error via `convert_error` — so the generated
        // `ZError` handle class + destructor are dead code, but this is the
        // idiomatic "ZError is FFI-representable" declaration (vs hand-written
        // dummy converters). `z_error_message` is the converter's accessor — it
        // must be declared `.fun_accessor` (which also exports it as `zErrorMessage`).
        // Canonical type representation: input (constructor) + output
        // (deconstructor) are declared on the `ptr_class` and AUTO-APPLY to every
        // matching param / return. `.ptr_class_input/output(...)` must precede any
        // `.fun`/`.fun_accessor` (those clear the ptr_class cursor).
        .package("errors")
        .ptr_class(pq!(ZError))
        // Canonical output: a ZError becomes its message string (1 leaf). Applies
        // to the `E` of every `Result<_, ZError>` (the `ze` error-callback arg).
        .ptr_class_output(pq!(z_error_message))
        .fun_accessor(pq!(z_error_message))
        .package("keyexpr")
        .ptr_class(pq!(ZKeyExpr))
        // Canonical input: a key-expr param accepts EITHER a String (built via
        // z_keyexpr_try_from) OR an existing handle (identity) — selector-
        // dispatched. Auto-applies to every ZKeyExpr param.
        .ptr_class_input(pq!(z_keyexpr_try_from))
        .ptr_class_input_direct()
        // Canonical output: a key-expr decomposes into BOTH its handle (identity)
        // and its borrowed string form (`z_keyexpr_as_str`) — 2 leaves ⇒ builder
        // callback. Auto-applies to every (non-Result) ZKeyExpr return.
        .ptr_class_output_direct()
        .ptr_class_output(pq!(z_keyexpr_as_str))
        .fun_accessor(pq!(z_keyexpr_as_str))
        // Fallible factories return `Result<ZKeyExpr, ZError>` (Result is not
        // output-decomposed → handle return + error callback).
        .fun(pq!(z_keyexpr_try_from))
        .fun(pq!(z_keyexpr_autocanonize))
        // Consumers: a/b key-expr params auto-constructed by the canonical input.
        .fun(pq!(z_keyexpr_intersects))
        .fun(pq!(z_keyexpr_includes))
        .fun(pq!(z_keyexpr_relation_to))
        .fun(pq!(z_keyexpr_join))
        .fun(pq!(z_keyexpr_concat))
        // Read accessors — `.fun_accessor` keeps `ke` handle-only (canonical
        // input/output composer skips fun_accessor fns).
        .fun_accessor(pq!(z_keyexpr_clone))
        .fun_accessor(pq!(z_keyexpr_to_string))
        .enum_class(pq!(SetIntersectionLevel))
        .package("config")
        .ptr_class(pq!(ZConfig))
        .fun(pq!(z_config_default))
        .fun(pq!(z_config_from_file))
        .fun(pq!(z_config_from_json))
        .fun(pq!(z_config_from_json5))
        .fun(pq!(z_config_from_yaml))
        .fun_accessor(pq!(z_config_get_json))
        .fun(pq!(z_config_insert_json5))
        .fun_accessor(pq!(z_config_clone))
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
        .fun_accessor(pq!(z_zenoh_id_to_bytes))
        .fun_accessor(pq!(z_zenoh_id_to_string))
        .package("scouting")
        .ptr_class(pq!(ZHello))
        // Canonical output: the scout callback decomposes a ZHello into its
        // three read fields in ONE crossing (no handle — read-only). Mirrors
        // ZSample's output expansion. Auto-applies to z_scout's `Fn(ZHello)`.
        .ptr_class_output(pq!(z_hello_whatami)) // WhatAmI enum -> Int
        .ptr_class_output(pq!(z_hello_zid)) // ZZenohId value_blob -> ByteArray
        .ptr_class_output(pq!(z_hello_locators)) // Vec<String> -> List<String>
        .fun_accessor(pq!(z_hello_whatami))
        .fun_accessor(pq!(z_hello_zid))
        .fun_accessor(pq!(z_hello_locators))
        .ptr_class(pq!(ZScout))
        .fun(pq!(z_scout))
        .package("logger")
        .fun(pq!(init_android_logs))
        .fun(pq!(try_init_zenoh_logs_from_env))
        .fun(pq!(init_zenoh_logs_from_env_or))
        .package("qos")
        .enum_class(pq!(Reliability))
        .enum_class(pq!(Priority))
        .enum_class(pq!(CongestionControl))
        .package("bytes")
        .ptr_class(pq!(ZZBytes))
        // Canonical input: `payload`/`attachment` params accept a `ByteArray`
        // (built via z_zbytes_from_vec). Canonical output: a ZZBytes is its bytes
        // (1 leaf ⇒ return value). `z_zbytes_from_slice(&[u8])` has no JNI shape.
        .ptr_class_input(pq!(z_zbytes_from_vec))
        .ptr_class_output(pq!(z_zbytes_to_bytes))
        .fun_accessor(pq!(z_zbytes_to_bytes))
        .fun_accessor(pq!(z_zbytes_clone))
        // Keep the factory exported with a raw-handle return (fun_output_direct
        // opts out of the canonical ByteArray decomposition).
        .fun(pq!(z_zbytes_from_vec))
        .fun_output_direct()
        .ptr_class(pq!(ZEncoding))
        // Canonical input: encoding params accept a `String` (z_encoding_from_string).
        // Canonical output: an encoding is its canonical string (1 leaf ⇒ return).
        .ptr_class_input(pq!(z_encoding_from_string))
        .ptr_class_output(pq!(z_encoding_to_string))
        .fun_accessor(pq!(z_encoding_id))
        .fun_accessor(pq!(z_encoding_schema))
        .fun_accessor(pq!(z_encoding_to_string))
        .fun_accessor(pq!(z_encoding_clone))
        // Factory: keep the handle return (canonical output would make it String).
        .fun(pq!(z_encoding_from_string))
        .fun_output_direct()
        .fun_accessor(pq!(z_encoding_with_schema))
        .fun(pq!(z_encoding_zenoh_bytes))
        .fun(pq!(z_encoding_zenoh_string))
        .fun(pq!(z_encoding_zenoh_serialized))
        .fun(pq!(z_encoding_application_octet_stream))
        .fun(pq!(z_encoding_text_plain))
        .fun(pq!(z_encoding_application_json))
        .fun(pq!(z_encoding_text_json))
        .fun(pq!(z_encoding_application_cdr))
        .fun(pq!(z_encoding_application_cbor))
        .fun(pq!(z_encoding_application_yaml))
        .fun(pq!(z_encoding_text_yaml))
        .fun(pq!(z_encoding_text_json5))
        .fun(pq!(z_encoding_application_python_serialized_object))
        .fun(pq!(z_encoding_application_protobuf))
        .fun(pq!(z_encoding_application_java_serialized_object))
        .fun(pq!(z_encoding_application_openmetrics_text))
        .fun(pq!(z_encoding_image_png))
        .fun(pq!(z_encoding_image_jpeg))
        .fun(pq!(z_encoding_image_gif))
        .fun(pq!(z_encoding_image_bmp))
        .fun(pq!(z_encoding_image_webp))
        .fun(pq!(z_encoding_application_xml))
        .fun(pq!(z_encoding_application_x_www_form_urlencoded))
        .fun(pq!(z_encoding_text_html))
        .fun(pq!(z_encoding_text_xml))
        .fun(pq!(z_encoding_text_css))
        .fun(pq!(z_encoding_text_javascript))
        .fun(pq!(z_encoding_text_markdown))
        .fun(pq!(z_encoding_text_csv))
        .fun(pq!(z_encoding_application_sql))
        .fun(pq!(z_encoding_application_coap_payload))
        .fun(pq!(z_encoding_application_json_patch_json))
        .fun(pq!(z_encoding_application_json_seq))
        .fun(pq!(z_encoding_application_jsonpath))
        .fun(pq!(z_encoding_application_jwt))
        .fun(pq!(z_encoding_application_mp4))
        .fun(pq!(z_encoding_application_soap_xml))
        .fun(pq!(z_encoding_application_yang))
        .fun(pq!(z_encoding_audio_aac))
        .fun(pq!(z_encoding_audio_flac))
        .fun(pq!(z_encoding_audio_mp4))
        .fun(pq!(z_encoding_audio_ogg))
        .fun(pq!(z_encoding_audio_vorbis))
        .fun(pq!(z_encoding_video_h261))
        .fun(pq!(z_encoding_video_h263))
        .fun(pq!(z_encoding_video_h264))
        .fun(pq!(z_encoding_video_h265))
        .fun(pq!(z_encoding_video_h266))
        .fun(pq!(z_encoding_video_mp4))
        .fun(pq!(z_encoding_video_ogg))
        .fun(pq!(z_encoding_video_raw))
        .fun(pq!(z_encoding_video_vp8))
        .fun(pq!(z_encoding_video_vp9))
        .package("time")
        .ptr_class(pq!(ZTimestamp))
        // Canonical output: a timestamp is its NTP64 value (`z_timestamp_ntp64`
        // → i64 → Long, 1 leaf). When nested in ZSample it contributes the Long
        // leaf; a `ZTimestamp`/`Option<&ZTimestamp>` return decomposes to `Long?`.
        .ptr_class_output(pq!(z_timestamp_ntp64))
        .fun_accessor(pq!(z_timestamp_ntp64))
        .fun_accessor(pq!(z_timestamp_id))
        .package("sample")
        .enum_class(pq!(SampleKind))
        .ptr_class(pq!(ZSample))
        // Canonical INPUT: identity only — a `ZSample` parameter takes the owned
        // handle directly. (The full-options constructors z_sample_put /
        // z_sample_delete carry `Option<ptr_class>` params — encoding/attachment —
        // which the recursive-input fold can't build through, so ZSample is not a
        // build-from-leaves input; it is constructed via the standalone `.fun`
        // constructors below and consumed by handle.)
        .ptr_class_input_direct()
        // Canonical output: the full sample decomposed in ONE crossing. Each
        // record is unwrapped per its return type's canonical output —
        // z_sample_key_expr → (ZKeyExpr handle, String); payload/attachment →
        // ByteArray (ZZBytes); encoding → String (ZEncoding); timestamp → Long?
        // (ZTimestamp); kind/priority/congestion/reliability → Int (enum);
        // express → Boolean; source_zid → ByteArray? (ZZenohId value_blob);
        // source_eid → Int; source_sn → Long. 13 records ⇒ many leaves ⇒ builder
        // callback. Auto-applies to every (non-Result) ZSample return (e.g.
        // z_reply_sample's `Option<&ZSample>`).
        .ptr_class_output(pq!(z_sample_key_expr))
        .ptr_class_output(pq!(z_sample_payload))
        .ptr_class_output(pq!(z_sample_encoding))
        .ptr_class_output(pq!(z_sample_kind))
        .ptr_class_output(pq!(z_sample_timestamp))
        .ptr_class_output(pq!(z_sample_express))
        .ptr_class_output(pq!(z_sample_priority))
        .ptr_class_output(pq!(z_sample_congestion_control))
        .ptr_class_output(pq!(z_sample_attachment))
        .ptr_class_output(pq!(z_sample_reliability))
        .ptr_class_output(pq!(z_sample_source_zid))
        .ptr_class_output(pq!(z_sample_source_eid))
        .ptr_class_output(pq!(z_sample_source_sn))
        // All sample accessors are record sources (fun_accessor): standalone they
        // stay handle/raw; decomposition happens via the canonical output above.
        .fun_accessor(pq!(z_sample_key_expr))
        .fun_accessor(pq!(z_sample_payload))
        .fun_accessor(pq!(z_sample_encoding))
        .fun_accessor(pq!(z_sample_kind))
        .fun_accessor(pq!(z_sample_timestamp))
        .fun_accessor(pq!(z_sample_express))
        .fun_accessor(pq!(z_sample_priority))
        .fun_accessor(pq!(z_sample_congestion_control))
        .fun_accessor(pq!(z_sample_attachment))
        .fun_accessor(pq!(z_sample_reliability))
        .fun_accessor(pq!(z_sample_source_zid))
        .fun_accessor(pq!(z_sample_source_eid))
        .fun_accessor(pq!(z_sample_source_sn))
        // Standalone sample constructors (callable from Kotlin). z_sample_put is
        // also the canonical input above (precedent: z_keyexpr_try_from is both
        // ptr_class_input and fun); z_sample_delete builds a Delete-kind sample.
        .fun(pq!(z_sample_put))
        .fun(pq!(z_sample_delete))
        // Below: key_expr / payload / attachment / encoding params are
        // auto-constructed by their types' canonical inputs (no per-fn calls).
        .package("pubsub")
        .ptr_class(pq!(ZPublisher))
        .fun(pq!(z_publisher_put))
        .fun(pq!(z_publisher_delete))
        .ptr_class(pq!(ZSubscriber))
        .package("query")
        .ptr_class(pq!(ZQueryable))
        .ptr_class(pq!(ZQuerier))
        .fun(pq!(z_querier_get))
        .enum_class(pq!(ReplyKeyExpr))
        .enum_class(pq!(QueryTarget))
        .enum_class(pq!(ConsolidationMode))
        .ptr_class(pq!(ZQuery))
        // Canonical output: the queryable callback decomposes a ZQuery into BOTH
        // its read fields AND the owned handle (identity), in ONE crossing. Like
        // ZSample's output expansion, but with `.ptr_class_output_direct()` so the
        // consumer keeps the handle to reply (`z_query_reply_*`) after the
        // callback returns — a Query must outlive the callback to be answered.
        // key_expr -> (ZKeyExpr handle + String); payload/attachment -> ByteArray?
        // (ZZBytes); encoding -> String? (ZEncoding); parameters -> String;
        // accepts_replies -> Int (enum). Auto-applies to z_session_declare_queryable's
        // `Fn(ZQuery)`. The `&ZQuery` INPUT of z_query_reply_* is unaffected.
        //
        // ORDER MATTERS: `.ptr_class_output_direct()` MUST be LAST. The root
        // identity moves the owned query (`Box::into_raw(Box::new(query))`) while
        // the nested ZKeyExpr identity (from z_query_keyexpr) clones from a borrow
        // of the same query; the emitter emits identity leaves in declaration
        // order, so the root move must follow the nested borrow to avoid a
        // "use of moved value" error.
        .ptr_class_output(pq!(z_query_keyexpr))
        .ptr_class_output(pq!(z_query_parameters))
        .ptr_class_output(pq!(z_query_payload))
        .ptr_class_output(pq!(z_query_encoding))
        .ptr_class_output(pq!(z_query_attachment))
        .ptr_class_output(pq!(z_query_accepts_replies))
        .ptr_class_output_direct()
        .fun(pq!(z_query_reply_success))
        .fun(pq!(z_query_reply_error))
        .fun(pq!(z_query_reply_delete))
        // z_query_reply_sample takes the sample by owned handle (ZSample's
        // canonical input is identity — see the sample package above).
        .fun(pq!(z_query_reply_sample))
        .fun_accessor(pq!(z_query_keyexpr))
        .fun_accessor(pq!(z_query_parameters))
        .fun_accessor(pq!(z_query_payload))
        .fun_accessor(pq!(z_query_encoding))
        .fun_accessor(pq!(z_query_attachment))
        .fun_accessor(pq!(z_query_accepts_replies))
        .ptr_class(pq!(ZReplyError))
        // Canonical output: a failed reply's error decomposed in one crossing —
        // payload → ByteArray (ZZBytes), encoding → String (ZEncoding).
        .ptr_class_output(pq!(z_reply_error_payload))
        .ptr_class_output(pq!(z_reply_error_encoding))
        .fun_accessor(pq!(z_reply_error_payload))
        .fun_accessor(pq!(z_reply_error_encoding))
        .ptr_class(pq!(ZReply))
        // Canonical output: the whole reply decomposed in ONE crossing, in the
        // current PRODUCT model — both arms' leaves are always in the
        // signature, the not-taken arm's are null. replier zid/eid + the
        // is_ok discriminator, then the `Option<&ZSample>` ok arm splices the
        // full sample (14 nullable leaves) and the `Option<&ZReplyError>` err
        // arm splices payload/encoding (2 nullable leaves) — 19 leaves total.
        // Auto-applies to the `Fn(ZReply)` reply callbacks of z_session_get /
        // z_querier_get / liveliness get; no identity record, so no ZReply
        // handle crosses (nothing for the consumer to close).
        .ptr_class_output(pq!(z_reply_replier_zid))
        .ptr_class_output(pq!(z_reply_replier_eid))
        .ptr_class_output(pq!(z_reply_is_ok))
        .ptr_class_output(pq!(z_reply_sample))
        .ptr_class_output(pq!(z_reply_err))
        .fun_accessor(pq!(z_reply_replier_zid))
        .fun_accessor(pq!(z_reply_replier_eid))
        .fun_accessor(pq!(z_reply_is_ok))
        // Record sources must be fun_accessor — z_reply_sample's standalone
        // export is therefore the cloned-handle form (no longer the
        // Sample-builder form; the decomposition above replaces it).
        .fun_accessor(pq!(z_reply_sample))
        .fun_accessor(pq!(z_reply_err))
        .package("liveliness")
        .ptr_class(pq!(ZLivelinessToken))
        .package("session")
        .ptr_class(pq!(ZSession))
        .fun(pq!(z_open))
        .fun(pq!(z_session_declare_publisher))
        .fun(pq!(z_session_put))
        .fun(pq!(z_session_delete))
        .fun(pq!(z_session_declare_subscriber))
        .fun(pq!(z_session_declare_querier))
        .fun(pq!(z_session_declare_queryable))
        .fun(pq!(z_session_declare_keyexpr))
        // z_session_undeclare_keyexpr: undeclaring needs the declared handle, not
        // a string — opt its key_expr param out of the canonical input.
        .fun(pq!(z_session_undeclare_keyexpr))
        .fun_input_direct(pq!(key_expr))
        .fun(pq!(z_session_get))
        .fun_accessor(pq!(z_session_zid))
        // Vec<ZZenohId>: ZZenohId is a value_blob (no canonical output), so these
        // return `List<ZZenohId>` via the normal Vec converter.
        .fun(pq!(z_session_peers_zid))
        .fun(pq!(z_session_routers_zid))
        // liveliness key_expr params auto-constructed by the canonical input.
        .fun(pq!(z_liveliness_declare_token))
        .fun(pq!(z_liveliness_get))
        .fun(pq!(z_liveliness_declare_subscriber));

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
