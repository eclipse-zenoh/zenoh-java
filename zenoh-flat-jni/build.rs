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
        .package_prefix("io.zenoh.jni") // the package of the generated JNI bindings
        .data_class(pq!(Error)) // structured Kotlin data class for Error
        .throwable()                    // …also throwable; JniExt's built-in
                                        // rank-2 Result<_, _> wrapper routes
                                        // Err(Error) through it on the JVM side.
        .ptr_class(pq!(ZKeyExpr))
        .class_object_fun(pq!(z_keyexpr_try_from))
        .class_object_fun(pq!(z_keyexpr_autocanonize))
        .class_fun(pq!(z_keyexpr_intersects))
        .class_fun(pq!(z_keyexpr_includes))
        .class_fun(pq!(z_keyexpr_relation_to))
        .class_fun(pq!(z_keyexpr_join))
        .class_fun(pq!(z_keyexpr_concat))
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
        .ptr_class(pq!(ZConfig))
        .class_object_fun(pq!(z_config_default))
        .class_object_fun(pq!(z_config_from_file))
        .class_object_fun(pq!(z_config_from_json))
        .class_object_fun(pq!(z_config_from_json5))
        .class_object_fun(pq!(z_config_from_yaml))
        .class_fun(pq!(z_config_get_json))
        .class_fun(pq!(z_config_insert_json5))
        .enum_class(pq!(WhatAmI))
        .ptr_class(pq!(ZZenohId))
        .class_fun(pq!(z_zenoh_id_to_bytes))
        .class_fun(pq!(z_zenoh_id_to_string))
        .value_class(pq!(ZenohId))
        .ptr_class(pq!(ZHello))
        .class_fun(pq!(z_hello_whatami))
        .class_fun(pq!(z_hello_zid))
        .class_fun(pq!(z_hello_locators))
        .data_class(pq!(Hello))
        .ptr_class(pq!(ZScout))
        .class_object_fun(pq!(z_scout))
        .class_object_fun(pq!(scout))
        .package("logger")
        .package_fun(pq!(init_android_logs))
        .package_fun(pq!(try_init_zenoh_logs_from_env))
        .package_fun(pq!(init_zenoh_logs_from_env_or))
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
