//! Build script — drives the four-step prebindgen-ext pipeline:
//!
//!   1. Scan `zenoh_flat`'s prebindgen source into a `Registry`.
//!   2. Resolve every type using `ZenohJniExt` (wraps the universal
//!      `JniExt` with zenoh-specific match arms).
//!   3. Write the generated Rust bindings to `zenoh_flat_jni.rs`.
//!   4. Write the generated Kotlin (per-callback fun-interface files +
//!      one aggregated `JNINative.kt`).

use std::path::PathBuf;

use proc_macro2::TokenStream;

use zenoh_flat::core::niches::Niches;
use zenoh_flat::core::prebindgen_ext::{ConverterImpl, PrebindgenExt};
use zenoh_flat::core::registry::{Registry, TypeKey};
use zenoh_flat::core::{resolve, write};
use zenoh_flat::jni::JniExt;
use zenoh_flat::kotlin::kotlin_ext::KotlinExt;
use zenoh_flat::kotlin::{KotlinInterfaceGenerator, KotlinTypeMap};

// ─────────────────────────────────────────────────────────────────────
// ZenohJniExt — thin wrapper that injects zenoh-specific arms before
// delegating to JniExt for every method.
// ─────────────────────────────────────────────────────────────────────

struct ZenohJniExt {
    base: JniExt,
}

impl ZenohJniExt {
    fn new(base: JniExt) -> Self {
        Self { base }
    }

    /// Wrap a `(wire, body, niches)` triple into a full `ConverterImpl`
    /// using the JniExt input wrapper convention. Most arms have no
    /// extra niche to declare beyond what the wire form implies, so we
    /// also offer the convenience [`Self::input_converter`] that fills
    /// `niches = Niches::empty()`.
    fn input_converter_with_niches(
        &self,
        ty: &syn::Type,
        wire: syn::Type,
        body: syn::Expr,
        niches: Niches,
    ) -> ConverterImpl {
        let function = self.base.input_wrapper(ty, &wire, &body);
        ConverterImpl {
            destination: wire,
            function,
            niches,
        }
    }

    /// Convenience: empty niches (no `Option<T>` cascade benefit).
    fn input_converter(
        &self,
        ty: &syn::Type,
        wire: syn::Type,
        body: syn::Expr,
    ) -> ConverterImpl {
        self.input_converter_with_niches(ty, wire, body, Niches::empty())
    }

    /// Output equivalent of [`Self::input_converter_with_niches`].
    fn output_converter_with_niches(
        &self,
        ty: &syn::Type,
        wire: syn::Type,
        body: syn::Expr,
        niches: Niches,
    ) -> ConverterImpl {
        let function = self.base.output_wrapper(ty, &wire, &body);
        ConverterImpl {
            destination: wire,
            function,
            niches,
        }
    }

    fn output_converter(
        &self,
        ty: &syn::Type,
        wire: syn::Type,
        body: syn::Expr,
    ) -> ConverterImpl {
        self.output_converter_with_niches(ty, wire, body, Niches::empty())
    }

    /// jint→enum decode helpers exposed by `crate::utils` in zenoh-jni.
    /// Wrapper takes v: &jint, but the decode helpers want a jint by value.
    fn jint_enum_decode(&self, ty_name: &str) -> Option<(syn::Type, syn::Expr)> {
        let path: syn::Path = match ty_name {
            "CongestionControl" => syn::parse_quote!(crate::utils::decode_congestion_control),
            "Priority"          => syn::parse_quote!(crate::utils::decode_priority),
            "Reliability"       => syn::parse_quote!(crate::utils::decode_reliability),
            "QueryTarget"       => syn::parse_quote!(crate::utils::decode_query_target),
            "ConsolidationMode" => syn::parse_quote!(crate::utils::decode_consolidation),
            "ReplyKeyExpr"      => syn::parse_quote!(crate::utils::decode_reply_key_expr),
            _ => return None,
        };
        Some((
            syn::parse_quote!(jni::sys::jint),
            syn::parse_quote!(#path(*v)?),
        ))
    }

    /// Manual callback overrides — pre-empt the auto-generated
    /// `process_kotlin_*_callback` for hand-written equivalents in
    /// zenoh-jni's `sample_callback` module.
    fn manual_callback_decode(&self, key: &str) -> Option<(syn::Type, syn::Expr)> {
        let path: syn::Path = match key {
            "impl Fn (Query) + Send + Sync + 'static" => {
                syn::parse_quote!(crate::sample_callback::process_kotlin_query_callback)
            }
            "impl Fn (Reply) + Send + Sync + 'static" => {
                syn::parse_quote!(crate::sample_callback::process_kotlin_reply_callback)
            }
            "impl Fn () + Send + Sync + 'static" => {
                syn::parse_quote!(crate::sample_callback::process_kotlin_on_close_callback)
            }
            _ => return None,
        };
        Some((
            syn::parse_quote!(jni::objects::JObject),
            syn::parse_quote!(#path(env, &v)?),
        ))
    }
}

impl PrebindgenExt for ZenohJniExt {
    // ── Setup — forward base prerequisites (OwnedObject etc.) ──

    fn install_prerequisites(&self, registry: &mut Registry) {
        self.base.install_prerequisites(registry);
    }

    // ── Item methods — delegate ──

    fn on_function(&self, f: &syn::ItemFn, registry: &Registry) -> TokenStream {
        self.base.on_function(f, registry)
    }
    fn on_struct(&self, s: &syn::ItemStruct, registry: &Registry) -> TokenStream {
        self.base.on_struct(s, registry)
    }
    fn on_enum(&self, e: &syn::ItemEnum, registry: &Registry) -> TokenStream {
        self.base.on_enum(e, registry)
    }
    fn on_const(&self, c: &syn::ItemConst, registry: &Registry) -> TokenStream {
        self.base.on_const(c, registry)
    }

    // ── Input rank-0 — zenoh-specific arms first, then delegate ──

    fn on_input_type_rank_0(&self, ty: &syn::Type, registry: &Registry) -> Option<ConverterImpl> {
        let key = TypeKey::from_type(ty).as_str().to_string();

        // jint→enum group
        if let Some(name) = bare_path_ident(ty) {
            if let Some((wire, body)) = self.jint_enum_decode(&name.to_string()) {
                return Some(self.input_converter(ty, wire, body));
            }
        }
        // Manual callback overrides
        if let Some((wire, body)) = self.manual_callback_decode(&key) {
            return Some(self.input_converter(ty, wire, body));
        }

        // Opaque Arc-handle inputs — universal "jlong-pointer-to-Arc"
        // convention via JniExt::opaque_arc_input (Clone-based) for
        // types that implement Clone (Session/Config/ZKeyExpr), and
        // JniExt::opaque_arc_borrow_input (OwnedObject-based) for
        // non-Clone handles like Publisher<'a>. Both expose the same
        // niche so `Option<T>` derives automatically; only the body
        // differs (clone vs OwnedObject<T>).
        for opaque_key in [
            "ZKeyExpr < 'static >",
            "Session",
            "Config",
        ] {
            if key == opaque_key {
                return Some(self.base.opaque_arc_input(ty));
            }
        }
        for opaque_key in ["Publisher < 'static >"] {
            if key == opaque_key {
                return Some(self.base.opaque_arc_borrow_input(ty));
            }
        }
        // Encoding (zenoh-specific)
        if key == "Encoding" {
            return Some(self.input_converter(
                ty,
                syn::parse_quote!(jni::objects::JObject),
                syn::parse_quote!(crate::utils::decode_jni_encoding(env, &v)?),
            ));
        }
        if key == "Option < Encoding >" {
            return Some(self.input_converter(
                ty,
                syn::parse_quote!(jni::objects::JObject),
                syn::parse_quote!(if !v.is_null() {
                    Some(crate::utils::decode_jni_encoding(env, &v)?)
                } else {
                    None
                }),
            ));
        }

        // Fall through to base
        self.base.on_input_type_rank_0(ty, registry)
    }

    fn on_input_type_rank_1(&self, pat: &syn::Type, t1: &syn::Type, registry: &Registry) -> Option<ConverterImpl> {
        self.base.on_input_type_rank_1(pat, t1, registry)
    }
    fn on_input_type_rank_2(&self, pat: &syn::Type, t1: &syn::Type, t2: &syn::Type, registry: &Registry) -> Option<ConverterImpl> {
        self.base.on_input_type_rank_2(pat, t1, t2, registry)
    }
    fn on_input_type_rank_3(&self, pat: &syn::Type, t1: &syn::Type, t2: &syn::Type, t3: &syn::Type, registry: &Registry) -> Option<ConverterImpl> {
        self.base.on_input_type_rank_3(pat, t1, t2, t3, registry)
    }

    // ── Output rank-0 — zenoh-specific arms first ──

    fn on_output_type_rank_0(&self, ty: &syn::Type, registry: &Registry) -> Option<ConverterImpl> {
        let key = TypeKey::from_type(ty).as_str().to_string();

        // Opaque Arc-handle outputs — universal jlong convention.
        // `Option<T>` derives automatically via the niche the helper
        // declares.
        for opaque_key in [
            "ZKeyExpr < 'static >",
            "Session",
            "Publisher < 'static >",
            "Subscriber < () >",
            "Querier < 'static >",
            "Queryable < () >",
            "AdvancedSubscriber < () >",
            "AdvancedPublisher < 'static >",
        ] {
            if key == opaque_key {
                return Some(self.base.opaque_arc_output(ty));
            }
        }
        // KeyExpr — auto-generated by JniExt's struct path
        if key == "KeyExpr" {
            return self.base.on_output_type_rank_0(ty, registry);
        }
        // SetIntersectionLevel — returned as jint via cast
        if key == "SetIntersectionLevel" {
            return Some(self.output_converter(
                ty,
                syn::parse_quote!(jni::sys::jint),
                syn::parse_quote!(v as jni::sys::jint),
            ));
        }
        // ZenohId → byte array
        if key == "ZenohId" {
            return Some(self.output_converter(
                ty,
                syn::parse_quote!(jni::sys::jbyteArray),
                syn::parse_quote!(crate::zenoh_id::zenoh_id_to_byte_array(env, v)?),
            ));
        }
        // Vec<ZenohId> → java.util.List<ByteArray>
        if key == "Vec < ZenohId >" {
            return Some(self.output_converter(
                ty,
                syn::parse_quote!(jni::sys::jobject),
                syn::parse_quote!(crate::zenoh_id::zenoh_ids_to_java_list(env, v)?),
            ));
        }

        self.base.on_output_type_rank_0(ty, registry)
    }

    fn on_output_type_rank_1(&self, pat: &syn::Type, t1: &syn::Type, registry: &Registry) -> Option<ConverterImpl> {
        self.base.on_output_type_rank_1(pat, t1, registry)
    }
    fn on_output_type_rank_2(&self, pat: &syn::Type, t1: &syn::Type, t2: &syn::Type, registry: &Registry) -> Option<ConverterImpl> {
        self.base.on_output_type_rank_2(pat, t1, t2, registry)
    }
    fn on_output_type_rank_3(&self, pat: &syn::Type, t1: &syn::Type, t2: &syn::Type, t3: &syn::Type, registry: &Registry) -> Option<ConverterImpl> {
        self.base.on_output_type_rank_3(pat, t1, t2, t3, registry)
    }
}

impl KotlinExt for ZenohJniExt {
    fn write_kotlin(
        &self,
        registry: &Registry,
        output_dir: &std::path::Path,
    ) -> Result<Vec<PathBuf>, zenoh_flat::kotlin::WriteKotlinError> {
        // Per-callback files come from the base JniExt's KotlinExt impl.
        self.base.write_kotlin(registry, output_dir)
    }
}

fn bare_path_ident(ty: &syn::Type) -> Option<syn::Ident> {
    if let syn::Type::Path(tp) = ty {
        if let Some(last) = tp.path.segments.last() {
            if matches!(last.arguments, syn::PathArguments::None) {
                return Some(last.ident.clone());
            }
        }
    }
    None
}

// ─────────────────────────────────────────────────────────────────────
// Pipeline driver
// ─────────────────────────────────────────────────────────────────────

fn shared_kotlin_types() -> KotlinTypeMap {
    KotlinTypeMap::new()
        .with_primitive_builtins()
        .add("String", "String")
        .add("Option<String>", "String")
        .add("Vec<u8>", "ByteArray")
        .add("Option<Vec<u8>>", "ByteArray")
        .add("CongestionControl", "Int")
        .add("Priority", "Int")
        .add("Reliability", "Int")
        .add("QueryTarget", "Int")
        .add("ConsolidationMode", "Int")
        .add("ReplyKeyExpr", "Int")
        .add("Option<ZKeyExpr<'static>>", "Long")
        .add("&KeyExpr", "KeyExpr")
        .add("ZResult<KeyExpr>", "KeyExpr")
        .add("ZResult<SetIntersectionLevel>", "Int")
        .add("Encoding", "io.zenoh.jni.JNIEncoding")
        .add("Option<Encoding>", "io.zenoh.jni.JNIEncoding")
        .add("&Session", "Long")
        .add("&Config", "Long")
        .add("Session", "Long")
        .add("ZResult<ZenohId>", "ByteArray")
        .add("ZResult<Vec<ZenohId>>", "List<ByteArray>")
        .add("ZResult<Session>", "Long")
        .add("ZResult<Publisher<'static>>", "Long")
        .add("ZResult<Subscriber<()>>", "Long")
        .add("ZResult<Querier<'static>>", "Long")
        .add("ZResult<Queryable<()>>", "Long")
        .add("ZResult<AdvancedSubscriber<()>>", "Long")
        .add("ZResult<AdvancedPublisher<'static>>", "Long")
        .add("ZResult<bool>", "Boolean")
}

fn main() {
    let source = prebindgen::Source::new(zenoh_flat::PREBINDGEN_OUT_DIR);

    // (1) Scan source.
    let mut registry = Registry::from_source(&source).expect("scan failed");

    // (2) Configure JniExt + ZenohJniExt and run rank-based resolution.
    let jni = JniExt::new()
        .source_module("zenoh_flat")
        .zresult("crate::errors::ZResult")
        .throw_macro("crate::throw_exception")
        .java_class_prefix("io/zenoh/jni")
        .jni_class_path("Java_io_zenoh_jni_JNINative")
        .jni_method_suffix("ViaJNI")
        .kotlin_callback_package("io.zenoh.jni.callbacks")
        .kotlin_callback_dir("../zenoh-jni-runtime/src/commonMain/kotlin/io/zenoh/jni/callbacks");
    let ext = ZenohJniExt::new(jni);
    resolve::resolve(&mut registry, &ext).expect("unresolved required types");

    // (3) Write Rust bindings file.
    let bindings_path = write::write_rust(&registry, &ext, "zenoh_flat_jni.rs")
        .expect("failed to write bindings");
    println!(
        "cargo:warning=Generated bindings at: {}",
        bindings_path.display()
    );

    // (4a) Per-callback Kotlin fun-interface files.
    let _ = KotlinExt::write_kotlin(
        &ext,
        &registry,
        std::path::Path::new("../zenoh-jni-runtime/src/commonMain/kotlin/io/zenoh/jni/callbacks"),
    )
    .expect("failed to write Kotlin callback files");

    // (4b) Aggregated JNINative.kt — uses the existing
    //      KotlinInterfaceGenerator. Until it is migrated to consume the
    //      new Registry, this step is a placeholder; the consumer's old
    //      Kotlin pipeline still produces the file.
    //      TODO: rewrite KotlinInterfaceGenerator to read the new Registry
    //      then call it here. For now, print a reminder.
    let _ = (shared_kotlin_types, KotlinInterfaceGenerator::builder);
    println!(
        "cargo:warning=Aggregated JNINative.kt generation is not yet wired \
         to the new Registry. Skipping."
    );
}
