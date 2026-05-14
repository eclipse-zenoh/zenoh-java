//! Rust file emission for the resolved `Registry`.
//!
//! `write_rust` collects every resolved input/output converter (each entry
//! already carries its full `ItemFn`), every per-item `on_<kind>` output,
//! and every passthrough item; concatenates them; and hands them to
//! `prebindgen::collect::Destination::write` (which does prettyplease
//! formatting and resolves the path against `OUT_DIR`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use prebindgen::collect::Destination;
use proc_macro2::TokenStream;

use crate::core::prebindgen_ext::PrebindgenExt;
use crate::core::registry::{Registry, TypeEntry, TypeKey};

/// Errors surfaced by the file-emission phase.
#[derive(Debug)]
pub enum WriteError {
    /// A `TokenStream` produced by an `on_*` trait method failed to parse
    /// as `syn::Item`s. Indicates a codegen bug in the ext.
    BadTokens(syn::Error),
}

impl std::fmt::Display for WriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WriteError::BadTokens(e) => write!(f, "generated tokens did not parse: {}", e),
        }
    }
}

impl std::error::Error for WriteError {}

/// Emit the resolved registry to a Rust file.
///
/// `out_path` may be relative (resolved against `OUT_DIR` by prebindgen) or
/// absolute. Returns the path actually written.
pub fn write_rust<P: AsRef<Path>, E: PrebindgenExt>(
    registry: &Registry,
    ext: &E,
    out_path: P,
) -> Result<PathBuf, WriteError> {
    let mut items: Vec<syn::Item> = Vec::new();

    // 1. Auto-generated converter wrappers (sorted by ident, deduped).
    for (_, item_fn) in collect_converter_items(registry) {
        items.push(syn::Item::Fn(item_fn));
    }

    // 2. Per-item Rust output from the ext.
    items.extend(parse_items_from_tokens(
        registry
            .functions
            .values()
            .map(|(item, _)| ext.on_function(item, registry)),
    )?);
    items.extend(parse_items_from_tokens(
        registry
            .structs
            .values()
            .map(|(item, _)| ext.on_struct(item, registry)),
    )?);
    items.extend(parse_items_from_tokens(
        registry
            .enums
            .values()
            .map(|(item, _)| ext.on_enum(item, registry)),
    )?);
    items.extend(parse_items_from_tokens(
        registry
            .consts
            .values()
            .map(|(item, _)| ext.on_const(item, registry)),
    )?);

    // 3. Passthrough items verbatim.
    for (item, _) in &registry.passthrough {
        items.push(item.clone());
    }

    let dest: Destination = items.into_iter().collect();
    Ok(dest.write(out_path))
}

/// Walk both type tables, dedupe each entry's stored `function` by name,
/// sort for determinism. Names are read directly off `entry.function.sig.ident`
/// — the plugin owns the naming.
pub fn collect_converter_items(registry: &Registry) -> Vec<(syn::Ident, syn::ItemFn)> {
    let mut by_name: BTreeMap<String, (syn::Ident, syn::ItemFn)> = BTreeMap::new();
    walk_resolved(&registry.input_types, |_, entry| {
        let name = entry.function.sig.ident.clone();
        by_name
            .entry(name.to_string())
            .or_insert((name, entry.function.clone()));
    });
    walk_resolved(&registry.output_types, |_, entry| {
        let name = entry.function.sig.ident.clone();
        by_name
            .entry(name.to_string())
            .or_insert((name, entry.function.clone()));
    });
    by_name.into_values().collect()
}

fn walk_resolved<F: FnMut(&TypeKey, &TypeEntry)>(
    buckets: &[std::collections::HashMap<TypeKey, Option<TypeEntry>>; 4],
    mut f: F,
) {
    for bucket in buckets {
        let mut keys: Vec<&TypeKey> = bucket.keys().collect();
        keys.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        for key in keys {
            if let Some(Some(entry)) = bucket.get(key) {
                f(key, entry);
            }
        }
    }
}

/// Parse a per-item `TokenStream` (which may be empty) as a sequence of
/// `syn::Item`s. Empty token streams yield zero items.
fn parse_items_from_tokens<I: IntoIterator<Item = TokenStream>>(
    iter: I,
) -> Result<Vec<syn::Item>, WriteError> {
    let mut out = Vec::new();
    for ts in iter {
        if ts.is_empty() {
            continue;
        }
        let file: syn::File = syn::parse2(ts.clone()).map_err(WriteError::BadTokens)?;
        out.extend(file.items);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedup_and_sort() {
        let mut reg = Registry::default();
        let key_a = TypeKey::parse("u64");
        let key_b = TypeKey::parse("Sample");
        let wire: syn::Type = syn::parse_quote!(jni::sys::jlong);
        let wire2: syn::Type = syn::parse_quote!(jni::objects::JObject);

        reg.input_types[0].insert(
            key_a.clone(),
            Some(TypeEntry {
                destination: wire.clone(),
                function: syn::parse_quote!(
                    fn jlong_to_u64_aaaa(v: jni::sys::jlong) -> u64 { v as u64 }
                ),
                subs: vec![],
                required: true,
            }),
        );
        reg.input_types[0].insert(
            key_b.clone(),
            Some(TypeEntry {
                destination: wire2.clone(),
                function: syn::parse_quote!(
                    fn JObject_to_Sample_bbbb(v: jni::objects::JObject) -> Sample { decode_sample(v) }
                ),
                subs: vec![],
                required: true,
            }),
        );

        let items = collect_converter_items(&reg);
        assert_eq!(items.len(), 2);
        // Sorted ASCII: "JObject_to_Sample_bbbb" < "jlong_to_u64_aaaa"
        // (uppercase J < lowercase j).
        assert_eq!(items[0].0.to_string(), "JObject_to_Sample_bbbb");
        assert_eq!(items[1].0.to_string(), "jlong_to_u64_aaaa");
    }
}
