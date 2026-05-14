//! Rust file emission for the resolved `Registry`.
//!
//! `write_rust` collects every resolved input/output converter (wrapped via
//! `PrebindgenExt::wrap_input_converter` / `wrap_output_converter`), every
//! per-item `on_<kind>` output, and every passthrough item; concatenates
//! them; and hands them to `prebindgen::collect::Destination::write` (which
//! does prettyplease formatting and resolves the path against `OUT_DIR`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use prebindgen::collect::Destination;
use proc_macro2::TokenStream;

use crate::core::converter_name::{input_name, output_name};
use crate::core::prebindgen_ext::PrebindgenExt;
use crate::core::registry::{Registry, TypeEntry, TypeKey};

/// Errors surfaced by the file-emission phase.
#[derive(Debug)]
pub enum WriteError {
    /// A `TokenStream` produced by an `on_*` trait method or a wrapper fn
    /// failed to parse as `syn::Item`s. Indicates a codegen bug in the ext.
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
    for (_, item_fn) in collect_converter_items(registry, ext) {
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

/// Walk both type tables, build wrapper `ItemFn`s, dedupe by name, sort
/// for determinism.
pub fn collect_converter_items<E: PrebindgenExt>(
    registry: &Registry,
    ext: &E,
) -> Vec<(syn::Ident, syn::ItemFn)> {
    let mut by_name: BTreeMap<String, (syn::Ident, syn::ItemFn)> = BTreeMap::new();
    walk_resolved(&registry.input_types, |key, entry| {
        let rust = key.to_type();
        let name = input_name(&rust, &entry.destination);
        let item_fn = ext.wrap_input_converter(&name, &rust, &entry.destination, &entry.body);
        by_name.entry(name.to_string()).or_insert((name, item_fn));
    });
    walk_resolved(&registry.output_types, |key, entry| {
        let rust = key.to_type();
        let name = output_name(&rust, &entry.destination);
        let item_fn = ext.wrap_output_converter(&name, &rust, &entry.destination, &entry.body);
        by_name.entry(name.to_string()).or_insert((name, item_fn));
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
        // Wrap into a synthetic file to parse a sequence of items.
        let file: syn::File =
            syn::parse2(ts.clone()).map_err(|e| WriteError::BadTokens(e))?;
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
                body: syn::parse_quote!(v as u64),
                subs: vec![],
                required: true,
            }),
        );
        reg.input_types[0].insert(
            key_b.clone(),
            Some(TypeEntry {
                destination: wire2.clone(),
                body: syn::parse_quote!(decode_sample(v)),
                subs: vec![],
                required: true,
            }),
        );

        struct Stub;
        impl crate::core::prebindgen_ext::PrebindgenExt for Stub {
            fn on_function(&self, _: &syn::ItemFn, _: &Registry) -> proc_macro2::TokenStream { Default::default() }
            fn on_struct(&self, _: &syn::ItemStruct, _: &Registry) -> proc_macro2::TokenStream { Default::default() }
            fn on_enum(&self, _: &syn::ItemEnum, _: &Registry) -> proc_macro2::TokenStream { Default::default() }
            fn on_input_type_rank_0(&self, _: &syn::Type, _: &Registry) -> Option<(syn::Type, syn::Expr)> { None }
            fn on_input_type_rank_1(&self, _: &syn::Type, _: &syn::Type, _: &Registry) -> Option<(syn::Type, syn::Expr)> { None }
            fn on_input_type_rank_2(&self, _: &syn::Type, _: &syn::Type, _: &syn::Type, _: &Registry) -> Option<(syn::Type, syn::Expr)> { None }
            fn on_input_type_rank_3(&self, _: &syn::Type, _: &syn::Type, _: &syn::Type, _: &syn::Type, _: &Registry) -> Option<(syn::Type, syn::Expr)> { None }
            fn on_output_type_rank_0(&self, _: &syn::Type, _: &Registry) -> Option<(syn::Type, syn::Expr)> { None }
            fn on_output_type_rank_1(&self, _: &syn::Type, _: &syn::Type, _: &Registry) -> Option<(syn::Type, syn::Expr)> { None }
            fn on_output_type_rank_2(&self, _: &syn::Type, _: &syn::Type, _: &syn::Type, _: &Registry) -> Option<(syn::Type, syn::Expr)> { None }
            fn on_output_type_rank_3(&self, _: &syn::Type, _: &syn::Type, _: &syn::Type, _: &syn::Type, _: &Registry) -> Option<(syn::Type, syn::Expr)> { None }
            fn wrap_input_converter(&self, name: &syn::Ident, rust: &syn::Type, wire: &syn::Type, body: &syn::Expr) -> syn::ItemFn {
                syn::parse_quote!(fn #name(v: #wire) -> #rust { #body })
            }
            fn wrap_output_converter(&self, name: &syn::Ident, rust: &syn::Type, wire: &syn::Type, body: &syn::Expr) -> syn::ItemFn {
                syn::parse_quote!(fn #name(v: &#rust) -> #wire { #body })
            }
        }
        let items = collect_converter_items(&reg, &Stub);
        assert_eq!(items.len(), 2);
        // Input names use <wire>_to_<rust>_<hash>. Sorted ASCII:
        //   JObject_to_Sample_xxxx  (uppercase J < lowercase j)
        //   jlong_to_u64_xxxx
        assert!(items[0].0.to_string().starts_with("JObject_to_Sample_"));
        assert!(items[1].0.to_string().starts_with("jlong_to_u64_"));
    }
}
