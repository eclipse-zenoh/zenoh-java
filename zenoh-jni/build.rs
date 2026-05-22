//! Build script — declarative configuration of the prebindgen-ext
//! pipeline. Reads top-to-bottom as:
//!   1. Configure `JniExt` (Rust crate paths + per-type rules: opaque
//!      handles, jint enums, custom wrappers, callback overrides,
//!      data-class names, `impl Into<T>` arms).
//!   2. Scan `zenoh_flat`'s prebindgen source and write the generated
//!      Rust bindings (`zenoh_flat_jni.rs`).
//!   3. Write every Kotlin file under one root — `generated-kotlin/`
//!      (`JNI*Callback.kt`, `NativeHandle.kt`, typed-handle classes,
//!      `JNIWrappers.kt`).

use prebindgen_ext::core::prebindgen_ext::IntoSource;
use prebindgen_ext::core::registry::Registry;
use prebindgen_ext::jni::jni_ext::KotlinMeta;
use prebindgen_ext::jni::JniExt;
use syn::parse_quote;

fn main() {
    let jni = JniExt::new()
        .source_module("zenoh_flat")
        .package("io.zenoh.jni")
        .jni_method_suffix("ViaJNI")
        // Declares the domain exception `ZError`. Generates the
        // `io.zenoh.jni.ZError` Kotlin class and the
        // `crate::generated::throw_ZError(env, &err)` free function the
        // throwing converters below dispatch to. Built-in converter
        // failures and `NativeHandle`'s closed-handle access surface as
        // the framework `JniBindingError` instead (pre-registered).
        //
        // Throwing converters bind to this exception via
        // `*_wrapper_throwing("ZError", body)`, where `body` returns
        // `Result<ty, ZError>`. The framework knows nothing about
        // `Result`/`ZResult`; the binding spells each shape out here.
        // When `body` returns a rust type (e.g. `ZResult<T>` → `T`) the
        // framework auto-composes a value-inspecting peel stage onto that
        // type's own converter, so a `ZResult<Session>` raises `ZError`
        // on the peel and `JniBindingError` on the jlong marshalling.
        .kotlin_exception_class("zenoh_flat::errors::ZError")
        // ZResult<T> output: a throwing converter whose closure returns
        // the rust type `T` — so the framework auto-composes it as a
        // value-inspecting peel stage onto T's own output converter. The
        // peel raises `ZError`; T's marshalling raises `JniBindingError`;
        // the Kotlin `@Throws` lists both. `v: ZResult<T>` is already
        // `Result<T, ZError>`, the exact shape a throwing body must
        // return — so the body is just `v`.
        .output_wrapper_throwing(
            "ZResult < _ >",
            "ZError",
            |t: &syn::Type, _reg: &Registry<KotlinMeta>| {
                Some((t.clone(), parse_quote!(v)))
            },
        )
        .output_wrapper("()", |_reg: &Registry<KotlinMeta>| {
            Some((parse_quote!(()), parse_quote!({ v })))
        })
        // ── Kotlin classes — `kotlin_class` configures: jlong wire
        // (input + output), `Box::into_raw`/`Box::from_raw` lifecycle,
        // `instanceof` dispatch class, and the Kotlin parameter-type
        // name. A typed-handle Kotlin shell class is auto-emitted by
        // default; chain `.method(...)` (repeat for each promoted
        // instance method) to add methods, or `.suppress_kotlin_code()`
        // to opt out of emission when the `.kt` file is hand-written.
        // The Kotlin class name defaults to the Rust short-name; chain
        // `.kotlin_name("Foo")` to override (relative to the configured
        // package).
        .kotlin_class("Session")
        .kotlin_name("JNISession")
        .suppress_kotlin_code()
        .kotlin_class("Config")
        .kotlin_name("JNIConfig")
        .suppress_kotlin_code()
        .kotlin_class("ZKeyExpr<'static>")
        .kotlin_name("JNIKeyExpr")
        .method("try_from")
        .method("autocanonize")
        .method("intersects")
        .method("includes")
        .method("relation_to")
        .method("join")
        .method("concat")
        .kotlin_class("Publisher<'static>")
        .kotlin_name("JNIPublisher")
        .method("put_publisher")
        .method("delete_publisher")
        .kotlin_class("Subscriber<()>")
        .kotlin_name("JNISubscriber")
        .kotlin_class("Querier<'static>")
        .kotlin_name("JNIQuerier")
        .method("querier_get")
        .kotlin_class("Queryable<()>")
        .kotlin_name("JNIQueryable")
        .kotlin_class("Query")
        .kotlin_name("JNIQuery")
        .method("reply_success")
        .method("reply_error")
        .method("reply_delete")
        .kotlin_class("LivelinessToken")
        .kotlin_name("JNILivelinessToken")
        .kotlin_class("AdvancedSubscriber<()>")
        .kotlin_name("JNIAdvancedSubscriber")
        .suppress_kotlin_code()
        .kotlin_class("AdvancedPublisher<'static>")
        .kotlin_name("JNIAdvancedPublisher")
        .suppress_kotlin_code()
        .kotlin_class("MatchingListener")
        .kotlin_name("JNIMatchingListener")
        .kotlin_class("SampleMissListener")
        .kotlin_name("JNISampleMissListener")
        // ── jint-encoded enums — sugar over `input_wrapper` for the
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
        // ── Value-shaped custom converters. These call into zenoh code
        // that returns `ZResult<_>`, so they register via
        // `*_wrapper_throwing("ZError", …)`: the closure returns a wire
        // type (terminal converter) and the body IS the `ZResult` — no
        // `?`/`Ok` ceremony, the body↔exception coupling handles it. A
        // decode/encode failure surfaces as a zenoh error, not the
        // framework `JniBindingError`. (`SetIntersectionLevel` is an
        // infallible `as` cast, so it stays a non-throwing
        // `output_wrapper`.) Non-primitive wires chain `kotlin_name`;
        // primitive wires auto-derive via `kotlin_for_wire`.
        .input_wrapper_throwing("Encoding", "ZError", |_reg: &Registry<KotlinMeta>| {
            Some((
                parse_quote!(jni::objects::JObject),
                parse_quote!(crate::utils::decode_jni_encoding(env, &v)),
            ))
        })
        .kotlin_name("JNIEncoding")
        .input_wrapper_throwing(
            "Option<Encoding>",
            "ZError",
            |_reg: &Registry<KotlinMeta>| {
                Some((
                    parse_quote!(jni::objects::JObject),
                    parse_quote!(
                        if !v.is_null() {
                            crate::utils::decode_jni_encoding(env, &v).map(Some)
                        } else {
                            Ok(None)
                        }
                    ),
                ))
            },
        )
        .kotlin_name("JNIEncoding")
        .output_wrapper("SetIntersectionLevel", |_reg: &Registry<KotlinMeta>| {
            Some((
                parse_quote!(jni::sys::jint),
                parse_quote!(v as jni::sys::jint),
            ))
        })
        .output_wrapper_throwing("ZenohId", "ZError", |_reg: &Registry<KotlinMeta>| {
            Some((
                parse_quote!(jni::sys::jbyteArray),
                parse_quote!(crate::zenoh_id::zenoh_id_to_byte_array(env, v)),
            ))
        })
        .output_wrapper_throwing("Vec<ZenohId>", "ZError", |_reg: &Registry<KotlinMeta>| {
            Some((
                parse_quote!(jni::sys::jobject),
                parse_quote!(crate::zenoh_id::zenoh_ids_to_java_list(env, v)),
            ))
        })
        .with_kotlin_type("List<ByteArray>")
        // ── Manual callback overrides — replaces the auto-generated
        // `process_kotlin_*_callback` dispatcher with a hand-written
        // one and reroutes the Kotlin FQN.
        // The hand-written dispatcher fns
        // (`process_kotlin_query_callback`, `process_kotlin_reply_callback`)
        // return `ZResult<_>` natively — they do domain decoding — so use
        // `callback_input_throwing("ZError", …)`: the emitted converter is
        // typed `Result<_, ZError>` and its body is the dispatcher call
        // directly (no `?`/`Ok`). The auto `impl Fn(Sample)` callback,
        // which has no domain decode, keeps the framework default.
        .callback_input_throwing(
            "impl Fn(Query) + Send + Sync + 'static",
            "ZError",
            "crate::sample_callback::process_kotlin_query_callback",
        )
        .kotlin_name("JNIQueryableCallback")
        .callback_input_throwing(
            "impl Fn(Reply) + Send + Sync + 'static",
            "ZError",
            "crate::sample_callback::process_kotlin_reply_callback",
        )
        .kotlin_name("JNIGetCallback")
        .callback_kotlin_name(
            "impl Fn() + Send + Sync + 'static",
            "JNIOnCloseCallback",
        )
        // ── Kotlin names for hand-maintained data classes whose
        // converters auto-generate from the struct shape but whose
        // Kotlin form lives in JNINative.kt. Primitives, opaque
        // handles, `jint_enum`s, `with_kotlin_name`-tagged
        // decoders/encoders, and every `Option<T>` / `ZResult<T>` /
        // `&T` wrapper auto-derive their Kotlin names through the
        // [`KotlinMeta`] propagation in the rank-N handlers — no
        // build.rs entry needed.
        .kotlin_value_type("Sample")
        .kotlin_value_type("MissDetectionConfig")
        .kotlin_value_type("HistoryConfig")
        .kotlin_value_type("CacheConfig")
        .kotlin_value_type("RecoveryConfig")
        // ── impl Into<T> source arms.
        .into_sources(
            "ZKeyExpr<'static>",
            [
                IntoSource::borrow(parse_quote!(ZKeyExpr<'static>)),
                IntoSource::borrow(parse_quote!(String)),
            ],
        );

    // ── Write Rust bindings ───────────────────────────────────────────
    // Single file: `zenoh_flat_jni.rs` carries the converters, the JNI
    // extern wrappers, AND the throw fns generated for every registered
    // `kotlin_exception_class` (emitted from `JniExt::prerequisites`).
    // The throw fns sit at the top of the file so wrapper code below
    // references them by bare name; external modules in the binding
    // crate reach them via the include module (e.g.
    // `crate::generated::throw_ZError`).
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
    // All generated Kotlin lives under `generated-kotlin/`; the runtime
    // module's Gradle source set picks it up via
    // `kotlin.srcDir("$rootDir/zenoh-jni/generated-kotlin")`.
    let kotlin_root = std::path::Path::new("generated-kotlin");
    for path in jni
        .write_kotlin(&registry, kotlin_root)
        .expect("write kotlin failed")
    {
        println!("cargo:warning=Wrote {}", path.display());
    }
}
