//! Build script — declarative configuration of the prebindgen-ext
//! pipeline. Reads top-to-bottom as:
//!   1. Configure `JniExt` (Rust crate paths + per-type rules: opaque
//!      handles, jint enums, custom decoders, callback overrides,
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

fn main() {
    let jni = JniExt::new()
        .source_module("zenoh_flat")
        // Bring `zenoh-flat`'s `ZResult<T>` alias into scope at the
        // generated file: the source `#[prebindgen]` fns return it by
        // short name, and the `output_throws("ZResult < _ >", ...)`
        // registration below takes a `v: ZResult<T>` parameter, so the
        // identifier must resolve in the include site. The framework
        // itself stays unaware of any specific Result-shape alias.
        .add_prerequisite(syn::parse_quote!(use zenoh_flat::errors::ZResult;))
        .package("io.zenoh.jni")
        .jni_method_suffix("ViaJNI")
        // Single error type spliced into every generated `Result<_, _>`
        // signature; must impl `From<String>` (see `zenoh-flat/src/errors.rs`).
        // Also generates the `io.zenoh.jni.ZError` Kotlin class and the
        // crate-root `crate::throw_ZError(env, &err)` free function the
        // wrappers below dispatch to. NativeHandle's closed-handle
        // exception is the primary (first-registered) here.
        //
        // Chained `.output_throws(...)` registrations are scoped to this
        // exception. The framework knows nothing about `Result`/`ZResult`;
        // the binding spells each shape out here:
        //   * `ZResult<T>` peels via `?` and delegates to T's inner output
        //     converter — same body the framework used to hardcode.
        //   * `()` is the identity passthrough for void-returning JNI
        //     externs (its throw action catches input-decode `?`
        //     failures, since the converter body itself never errors).
        .kotlin_exception_class("zenoh_flat::errors::ZError")
        .output_throws(
            "ZResult < _ >",
            |t: &syn::Type, reg: &Registry<KotlinMeta>| {
                let inner = reg.output_entry(t)?;
                let inner_wire = inner.destination.clone();
                let inner_conv = inner.function.sig.ident.clone();
                Some((
                    inner_wire,
                    syn::parse_quote!({
                        let __i = v?;
                        #inner_conv(env, __i)?
                    }),
                ))
            },
        )
        .output_throws("()", |_reg: &Registry<KotlinMeta>| {
            Some((syn::parse_quote!(()), syn::parse_quote!({ v })))
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
        .kotlin_name("JNIEncoding")
        .input_decoder(
            "Option<Encoding>",
            "jni::objects::JObject",
            "if !v.is_null() { Some(crate::utils::decode_jni_encoding(env, &v)?) } else { None }",
        )
        .kotlin_name("JNIEncoding")
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
        .with_kotlin_type("List<ByteArray>")
        // ── Manual callback overrides — replaces the auto-generated
        // `process_kotlin_*_callback` dispatcher with a hand-written
        // one and reroutes the Kotlin FQN.
        .callback_input(
            "impl Fn(Query) + Send + Sync + 'static",
            "crate::sample_callback::process_kotlin_query_callback",
        )
        .kotlin_name("JNIQueryableCallback")
        .callback_input(
            "impl Fn(Reply) + Send + Sync + 'static",
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
                IntoSource::borrow(syn::parse_quote!(ZKeyExpr<'static>)),
                IntoSource::borrow(syn::parse_quote!(String)),
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
