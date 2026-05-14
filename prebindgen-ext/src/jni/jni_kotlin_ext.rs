//! `KotlinExt` impl for [`JniExt`].
//!
//! Today's pipeline emits two kinds of Kotlin output:
//! 1. One aggregated `JNINative.kt` (interface + data classes + external
//!    funs). This is currently produced by the existing
//!    [`crate::kotlin::KotlinInterfaceGenerator`] called separately from
//!    `build.rs`.
//! 2. One `JNI<Stem>Callback.kt` per `impl Fn(args) + Send + Sync + 'static`
//!    type encountered. These get emitted here via `JniExt::write_kotlin`.
//!
//! The split is deliberate: the per-callback files are the new artifact
//! introduced by the rewrite; the aggregated interface remains the
//! responsibility of the existing generator and is not touched by JniExt.

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use quote::ToTokens;

use crate::core::registry::{extract_fn_trait_args, Registry, TypeKey};
use crate::jni::jni_ext::JniExt;
use crate::kotlin::kotlin_ext::{KotlinExt, KotlinFile, WriteKotlinError};
use crate::kotlin::type_map::KotlinTypeMap;

impl KotlinExt for JniExt {
    fn write_kotlin(
        &self,
        registry: &Registry,
        output_dir: &Path,
    ) -> Result<Vec<PathBuf>, WriteKotlinError> {
        // Iterate every resolved type entry in either direction and look for
        // impl Fn(...) wires. Deduplicate by canonical type key.
        let mut seen: HashSet<TypeKey> = HashSet::new();
        let mut written = Vec::new();
        let target_dir = if !self.kotlin_callback_dir.as_os_str().is_empty() {
            self.kotlin_callback_dir.clone()
        } else {
            output_dir.to_path_buf()
        };

        for buckets in [&registry.input_types, &registry.output_types] {
            for bucket in buckets.iter() {
                for (key, slot) in bucket {
                    if slot.is_none() {
                        continue;
                    }
                    if !seen.insert(key.clone()) {
                        continue;
                    }
                    let ty = key.to_type();
                    if let Some(args) = extract_fn_trait_args(&ty) {
                        let file = build_callback_kotlin_file(self, &args, registry);
                        // Write directly under target_dir (which is already
                        // the package-qualified callbacks directory),
                        // bypassing KotlinFile::write's package-nesting.
                        std::fs::create_dir_all(&target_dir)?;
                        let path = target_dir.join(format!("{}.kt", file.class_name));
                        std::fs::write(&path, &file.contents)?;
                        written.push(path);
                    }
                }
            }
        }
        Ok(written)
    }
}

impl JniExt {
    /// Return the `<rust-type-key> → <kotlin FQN>` map for every
    /// `impl Fn(args)` type the Registry has resolved. Use this to merge
    /// into a `KotlinTypeMap` consumed by the aggregated-interface
    /// generator (so it can refer to callbacks by their Kotlin FQN).
    pub fn collect_kotlin_callback_fqns(&self, registry: &Registry) -> KotlinTypeMap {
        let mut map = KotlinTypeMap::new();
        let mut seen: HashSet<TypeKey> = HashSet::new();
        for buckets in [&registry.input_types, &registry.output_types] {
            for bucket in buckets.iter() {
                for (key, slot) in bucket {
                    if slot.is_none() {
                        continue;
                    }
                    if !seen.insert(key.clone()) {
                        continue;
                    }
                    let ty = key.to_type();
                    if let Some(args) = extract_fn_trait_args(&ty) {
                        let stem = derive_callback_stem(&args);
                        let fqn = if self.kotlin_callback_package.is_empty() {
                            format!("JNI{}Callback", stem)
                        } else {
                            format!("{}.JNI{}Callback", self.kotlin_callback_package, stem)
                        };
                        map = map.add(key.as_str(), fqn);
                    }
                }
            }
        }
        map
    }
}

fn build_callback_kotlin_file(
    ext: &JniExt,
    args: &[syn::Type],
    registry: &Registry,
) -> KotlinFile {
    let stem = derive_callback_stem(args);
    let class_name = format!("JNI{}Callback", stem);
    let package = ext.kotlin_callback_package.clone();

    let kotlin_types = ext.collect_kotlin_callback_fqns(registry);

    // Resolve each arg's Kotlin type. Falls back to the bare last-segment
    // ident when not found in the map (matches today's
    // CallbacksConverter::emit_for_signature lookup behavior).
    let mut params: Vec<String> = Vec::new();
    let mut used_fqns: BTreeSet<String> = BTreeSet::new();
    for (i, arg) in args.iter().enumerate() {
        let canon = arg.to_token_stream().to_string();
        let kotlin_ty = kotlin_types
            .lookup(&canon)
            .map(str::to_string)
            .or_else(|| {
                if let syn::Type::Path(tp) = arg {
                    if let Some(last) = tp.path.segments.last() {
                        return Some(last.ident.to_string());
                    }
                }
                None
            })
            .unwrap_or_else(|| "Any".to_string());
        let short = register_fqn(&kotlin_ty, &mut used_fqns);
        let optional_suffix = if is_option_type(arg) { "?" } else { "" };
        params.push(format!("        p{i}: {short}{optional_suffix},"));
    }

    let contents = render_kotlin_interface(&package, &class_name, &params, &used_fqns);
    KotlinFile {
        package,
        class_name,
        contents,
    }
}

fn render_kotlin_interface(
    package: &str,
    class_name: &str,
    params: &[String],
    used_fqns: &BTreeSet<String>,
) -> String {
    let mut imports: Vec<String> = used_fqns
        .iter()
        .filter(|fqn| {
            let pkg = fqn.rsplit_once('.').map(|(p, _)| p).unwrap_or("");
            !pkg.is_empty() && pkg != package
        })
        .cloned()
        .collect();
    imports.sort();
    imports.dedup();

    let mut out = String::new();
    out.push_str("// Auto-generated by JniExt — do not edit by hand.\n");
    if !package.is_empty() {
        out.push_str(&format!("package {}\n\n", package));
    }
    for imp in &imports {
        out.push_str(&format!("import {}\n", imp));
    }
    if !imports.is_empty() {
        out.push('\n');
    }
    out.push_str(&format!("public fun interface {} {{\n", class_name));
    if params.is_empty() {
        out.push_str("    fun run()\n");
    } else {
        out.push_str("    fun run(\n");
        for p in params {
            out.push_str(p);
            out.push('\n');
        }
        out.push_str("    )\n");
    }
    out.push_str("}\n");
    out
}

fn derive_callback_stem(args: &[syn::Type]) -> String {
    if args.is_empty() {
        return "Empty".into();
    }
    let mut s = String::new();
    for a in args {
        s.push_str(&type_short_ident(a));
    }
    s
}

fn type_short_ident(ty: &syn::Type) -> String {
    if let syn::Type::Path(tp) = ty {
        if let Some(last) = tp.path.segments.last() {
            return last.ident.to_string();
        }
    }
    "Unknown".into()
}

fn is_option_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(tp) = ty {
        if let Some(last) = tp.path.segments.last() {
            return last.ident == "Option";
        }
    }
    false
}

fn register_fqn(fqn: &str, used: &mut BTreeSet<String>) -> String {
    if fqn.contains('.') {
        used.insert(fqn.to_string());
        fqn.rsplit('.').next().unwrap_or(fqn).to_string()
    } else {
        fqn.to_string()
    }
}
