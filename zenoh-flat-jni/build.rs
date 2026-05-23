use prebindgen_ext::core::registry::Registry;
use prebindgen_ext::jni::jni_ext::KotlinMeta;
use prebindgen_ext::jni::JniExt;
use syn::parse_quote as pq;

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
        .kotlin_package("keyexpr")
        .method("keyexpr_validate")
        ;

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