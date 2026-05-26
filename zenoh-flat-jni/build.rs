use prebindgen_ext::core::prebindgen_ext::IntoSource;
use prebindgen_ext::core::registry::Registry;
use prebindgen_ext::jni::JniExt;
use syn::parse_quote as pq;

fn fail(context: &str, err: impl std::fmt::Display) -> ! {
    eprintln!("error: prebindgen-ext {context}: {err}");
    std::process::exit(1);
}

fn main() {
    let jni = JniExt::new()
        .source_module(pq!(zenoh_flat)) // how to prefix prebindgen-marked items (functions, types
        .kotlin_package_prefix("io.zenoh.jni") // the package of the generated JNI bindings
        .kotlin_data_class(pq!(Error)) // structured Kotlin data class for Error
        .throwable()                    // …also throwable; JniExt's built-in
                                        // rank-2 Result<_, _> wrapper routes
                                        // Err(Error) through it on the JVM side.
        .kotlin_ptr_class(pq!(ZKeyExpr))
        .method("z_keyexpr_try_from")
        .method("z_keyexpr_autocanonize")
        .method("z_keyexpr_intersects")
        .method("z_keyexpr_includes")
        .method("z_keyexpr_relation_to")
        .method("z_keyexpr_join")
        .method("z_keyexpr_concat")
        .kotlin_data_class(pq!(KeyExpr))
        .method("keyexpr_try_from")
        .method("keyexpr_autocanonize")
        .method("keyexpr_intersects")
        .method("keyexpr_includes")
        .method("keyexpr_relation_to")
        .method("keyexpr_join")
        .method("keyexpr_concat")
        .into_sources(
            pq!(KeyExpr),
            [
                IntoSource::borrow(pq!(KeyExpr)),
                IntoSource::borrow(pq!(ZKeyExpr)),
                IntoSource::borrow(pq!(String)),
            ],
        )
        .kotlin_ptr_class(pq!(ZConfig))
        .method("z_config_default")
        .method("z_config_from_file")
        .method("z_config_from_json")
        .method("z_config_from_json5")
        .method("z_config_from_yaml")
        .method("z_config_get_json")
        .method("z_config_insert_json5")
        .kotlin_enum(pq!(WhatAmI))
        .kotlin_ptr_class(pq!(ZZenohId))
        .method("z_zenoh_id_to_bytes")
        .method("z_zenoh_id_to_string")
        .kotlin_value_class(pq!(ZenohId))
        .kotlin_ptr_class(pq!(ZHello))
        .method("z_hello_whatami")
        .method("z_hello_zid")
        .method("z_hello_locators")
        .kotlin_data_class(pq!(Hello))
        .kotlin_ptr_class(pq!(ZScout))
        .method("z_scout")
        .method("scout")
        .kotlin_package("logger")
        .method("init_android_logs")
        .method("try_init_zenoh_logs_from_env")
        .method("init_zenoh_logs_from_env_or")
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
    // `kotlin.srcDir("$rootDir/zenoh-jni/generated-kotlin")`.
    let kotlin_root = std::path::Path::new("generated-kotlin");
    for path in match jni.write_kotlin(&registry, kotlin_root) {
        Ok(paths) => paths,
        Err(err) => fail("write_kotlin failed", err),
    } {
        println!("cargo:warning=Wrote {}", path.display());
    }
}