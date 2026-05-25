use prebindgen_ext::core::prebindgen_ext::IntoSource;
use prebindgen_ext::core::registry::Registry;
use prebindgen_ext::jni::jni_ext::KotlinMeta;
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
        .kotlin_exception_class(pq!(Error)) // The prebindgen-marked type which can be thrown as java exception
        .output_wrapper(
            pq!(Result<_, Error>),
            |t: &syn::Type, _: &Registry<KotlinMeta>| {
                Some((
                    t.clone(), // the type R to be returned in case of success 
                    Some(pq!(Error)), // If Some(T) - assume the the next expr procuces Result<R,T>, 
                                      // and throws T in case of Err. If None - assume that the next expr 
                                      // produces R directly
                    pq!(v) // just pass the original Result<_, Error> as is, no transofmation
                ))
            },
        )
        .kotlin_ptr_class(pq!(ZKeyExpr))
        .method("z_keyexpr_try_from")
        .method("z_keyexpr_autocanonize")
        .method("z_keyexpr_intersects")
        .kotlin_data_class(pq!(KeyExpr)) 
        .method("keyexpr_try_from")
        .method("keyexpr_autocanonize")
        .method("keyexpr_intersects")
        .into_sources(
            pq!(KeyExpr),
            [
                IntoSource::borrow(pq!(KeyExpr)),
                IntoSource::borrow(pq!(ZKeyExpr)),
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
    // `kotlin.srcDir("$rootDir/zenoh-jni/generated-kotlin")`.
    let kotlin_root = std::path::Path::new("generated-kotlin");
    for path in match jni.write_kotlin(&registry, kotlin_root) {
        Ok(paths) => paths,
        Err(err) => fail("write_kotlin failed", err),
    } {
        println!("cargo:warning=Wrote {}", path.display());
    }
}