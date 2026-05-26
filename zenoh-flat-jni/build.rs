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
        .companion_method(pq!(z_keyexpr_try_from))
        .companion_method(pq!(z_keyexpr_autocanonize))
        .method(pq!(z_keyexpr_intersects))
        .method(pq!(z_keyexpr_includes))
        .method(pq!(z_keyexpr_relation_to))
        .method(pq!(z_keyexpr_join))
        .method(pq!(z_keyexpr_concat))
        .kotlin_data_class(pq!(KeyExpr))
        .companion_method(pq!(keyexpr_try_from))
        .companion_method(pq!(keyexpr_autocanonize))
        .companion_method(pq!(keyexpr_intersects))
        .companion_method(pq!(keyexpr_includes))
        .companion_method(pq!(keyexpr_relation_to))
        .companion_method(pq!(keyexpr_join))
        .companion_method(pq!(keyexpr_concat))
        .into_sources(
            pq!(KeyExpr),
            [
                IntoSource::borrow(pq!(KeyExpr)),
                IntoSource::borrow(pq!(ZKeyExpr)),
                IntoSource::borrow(pq!(String)),
            ],
        )
        .kotlin_ptr_class(pq!(ZConfig))
        .companion_method(pq!(z_config_default))
        .companion_method(pq!(z_config_from_file))
        .companion_method(pq!(z_config_from_json))
        .companion_method(pq!(z_config_from_json5))
        .companion_method(pq!(z_config_from_yaml))
        .method(pq!(z_config_get_json))
        .method(pq!(z_config_insert_json5))
        .kotlin_enum(pq!(WhatAmI))
        .kotlin_ptr_class(pq!(ZZenohId))
        .method(pq!(z_zenoh_id_to_bytes))
        .method(pq!(z_zenoh_id_to_string))
        .kotlin_value_class(pq!(ZenohId))
        .kotlin_ptr_class(pq!(ZHello))
        .method(pq!(z_hello_whatami))
        .method(pq!(z_hello_zid))
        .method(pq!(z_hello_locators))
        .kotlin_data_class(pq!(Hello))
        .kotlin_ptr_class(pq!(ZScout))
        .companion_method(pq!(z_scout))
        .companion_method(pq!(scout))
        .kotlin_package("logger")
        .function(pq!(init_android_logs))
        .function(pq!(try_init_zenoh_logs_from_env))
        .function(pq!(init_zenoh_logs_from_env_or))
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
