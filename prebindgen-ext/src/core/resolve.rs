//! Rank-based resolver and the post-resolution `required` propagation pass.
//!
//! The resolver fills `Registry::input_types` / `output_types` cells by
//! interrogating the language ext at successive rank phases:
//!   * Phase 0: `on_*_type_rank_0(ty)` is asked about every unresolved
//!              entry, regardless of the entry's own rank.
//!   * Phases 1..3: for each still-unresolved entry of rank ≥ N,
//!     [`enumerate_wildcard_subs`] yields all `(pattern, subs)` of size N
//!     and asks the matching rank-N method.
//!
//! Within each phase, a fixed-point sub-loop runs PASS A (read-only, build
//! deltas) then PASS B (apply deltas) until no entry advances. This handles
//! same-rank dependencies (e.g. `Vec<Option<u64>>` whose `Vec<_>` body
//! needs `Option<u64>`'s wire which is itself a rank-1 resolution).
//!
//! After all phases finish, [`propagate_required`] performs a BFS from the
//! scan-time required entries through `subs` edges; the final invariant is
//! that every `required: true && None` is reported as an error.
//!
//! Variant ordering within a single rank-N attempt is **deepest first**,
//! left-to-right; the first `Some` returned by the ext wins.

use std::collections::VecDeque;

use prebindgen::SourceLocation;

use crate::core::prebindgen_ext::PrebindgenExt;
use crate::core::registry::{
    immediate_subtype_positions, Direction, Registry, TypeEntry, TypeKey, MAX_RANK,
};

/// Errors surfaced by the resolution phase.
#[derive(Debug)]
pub enum ResolveError {
    /// A type that was scanned as required (or transitively reached from a
    /// required type via `subs`) ended up with no converter.
    Unresolved { entries: Vec<UnresolvedEntry> },
}

#[derive(Debug)]
pub struct UnresolvedEntry {
    pub key: TypeKey,
    pub direction: Direction,
    pub location: Option<SourceLocation>,
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolveError::Unresolved { entries } => {
                writeln!(f, "{} required type(s) could not be resolved:", entries.len())?;
                for e in entries {
                    let dir = match e.direction {
                        Direction::Input => "input",
                        Direction::Output => "output",
                    };
                    let loc = e
                        .location
                        .as_ref()
                        .map(|l| format!(" (first seen at {})", l))
                        .unwrap_or_default();
                    writeln!(f, "  - {} ({}){}", e.key, dir, loc)?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ResolveError {}

/// Top-level resolution entry point.
///
/// Runs ONE big fixed-point loop covering both directions and all ranks.
/// Each iteration sweeps every unresolved entry (both input and output) at
/// every rank; deltas are collected without mutating the registry, then
/// applied at the end of the iteration. Loops until a full sweep produces
/// zero deltas.
///
/// The single-loop design lets cross-direction dependencies converge: e.g.
/// `impl Fn(Sample)` is an INPUT entry whose callback wrapper needs
/// `Sample`'s OUTPUT converter (callback args flow Rust→Kotlin). Sample's
/// output resolves in the same iteration as everything else, then
/// `impl Fn(Sample)`'s rank-1 attempt succeeds in the next.
pub fn resolve<E: PrebindgenExt>(
    registry: &mut Registry,
    ext: &E,
) -> Result<(), ResolveError> {
    loop {
        let mut deltas_in: Vec<(usize, TypeKey, TypeEntry)> = Vec::new();
        let mut deltas_out: Vec<(usize, TypeKey, TypeEntry)> = Vec::new();
        for n in 0..=MAX_RANK {
            deltas_in.extend(collect_phase_deltas(registry, Direction::Input, n, ext));
            deltas_out.extend(collect_phase_deltas(registry, Direction::Output, n, ext));
        }
        if deltas_in.is_empty() && deltas_out.is_empty() {
            break;
        }
        apply_deltas(registry, Direction::Input, deltas_in);
        apply_deltas(registry, Direction::Output, deltas_out);
    }
    propagate_required(registry);
    final_invariant_check(registry)
}

/// PASS A — walk every unresolved entry in buckets `n..=MAX_RANK`, ask the
/// ext, collect successful results without mutating the registry.
fn collect_phase_deltas<E: PrebindgenExt>(
    registry: &Registry,
    dir: Direction,
    n: usize,
    ext: &E,
) -> Vec<(usize, TypeKey, TypeEntry)> {
    let mut deltas: Vec<(usize, TypeKey, TypeEntry)> = Vec::new();
    let buckets = match dir {
        Direction::Input => &registry.input_types,
        Direction::Output => &registry.output_types,
    };

    for bucket_idx in n..=MAX_RANK {
        for (key, slot) in &buckets[bucket_idx] {
            if slot.is_some() {
                continue;
            }
            let key_ty = key.to_type();
            let scan_required = match dir {
                Direction::Input => registry.is_required_input_at_scan(key),
                Direction::Output => registry.is_required_output_at_scan(key),
            };
            if let Some(entry) = try_resolve_entry(ext, &key_ty, n, dir, scan_required, registry)
            {
                deltas.push((bucket_idx, key.clone(), entry));
            }
        }
    }
    deltas
}

/// PASS B — apply collected deltas. Sole writer to the registry maps in
/// this iteration.
fn apply_deltas(
    registry: &mut Registry,
    dir: Direction,
    deltas: Vec<(usize, TypeKey, TypeEntry)>,
) {
    let buckets = match dir {
        Direction::Input => &mut registry.input_types,
        Direction::Output => &mut registry.output_types,
    };
    for (bucket_idx, key, entry) in deltas {
        if let Some(slot) = buckets[bucket_idx].get_mut(&key) {
            if slot.is_none() {
                *slot = Some(entry);
            }
        }
    }
}

/// Attempt to resolve one entry at exactly rank N (N=0 is whole-type;
/// N≥1 enumerates wildcard substitutions deepest-first).
fn try_resolve_entry<E: PrebindgenExt>(
    ext: &E,
    key_ty: &syn::Type,
    n: usize,
    dir: Direction,
    scan_required: bool,
    registry: &Registry,
) -> Option<TypeEntry> {
    if n == 0 {
        let res = match dir {
            Direction::Input => ext.on_input_type_rank_0(key_ty, registry),
            Direction::Output => ext.on_output_type_rank_0(key_ty, registry),
        };
        return res.map(|(wire, body)| TypeEntry {
            destination: wire,
            body,
            subs: vec![],
            required: scan_required,
        });
    }

    for (pattern, subs) in enumerate_wildcard_subs(key_ty, n) {
        let result = match (dir, n) {
            (Direction::Input, 1) => ext.on_input_type_rank_1(&pattern, &subs[0], registry),
            (Direction::Input, 2) => {
                ext.on_input_type_rank_2(&pattern, &subs[0], &subs[1], registry)
            }
            (Direction::Input, 3) => {
                ext.on_input_type_rank_3(&pattern, &subs[0], &subs[1], &subs[2], registry)
            }
            (Direction::Output, 1) => ext.on_output_type_rank_1(&pattern, &subs[0], registry),
            (Direction::Output, 2) => {
                ext.on_output_type_rank_2(&pattern, &subs[0], &subs[1], registry)
            }
            (Direction::Output, 3) => {
                ext.on_output_type_rank_3(&pattern, &subs[0], &subs[1], &subs[2], registry)
            }
            _ => unreachable!("rank N is bounded to 0..=3 by MAX_RANK"),
        };
        if let Some((wire, body)) = result {
            let sub_keys: Vec<TypeKey> = subs.iter().map(TypeKey::from_type).collect();
            return Some(TypeEntry {
                destination: wire,
                body,
                subs: sub_keys,
                required: scan_required,
            });
        }
    }
    None
}

// ──────────────────────────────────────────────────────────────────────
// Wildcard enumeration
// ──────────────────────────────────────────────────────────────────────

/// Yield every `(pattern, subs)` where `subs` is a set of `n` pairwise
/// non-overlapping positions from `ty`'s tree, and `pattern` is `ty` with
/// each chosen position replaced by `_`. Returned in **deepest-first**,
/// left-to-right document order.
pub fn enumerate_wildcard_subs(ty: &syn::Type, n: usize) -> Vec<(syn::Type, Vec<syn::Type>)> {
    if n == 0 {
        return vec![];
    }
    // Collect all substitutable position paths in the type tree.
    let mut paths: Vec<PositionPath> = Vec::new();
    collect_positions(ty, &mut Vec::new(), &mut paths);

    // Enumerate every size-n subset of paths.
    let mut variants: Vec<(usize, syn::Type, Vec<syn::Type>)> = Vec::new();
    for choice in choose_indices(paths.len(), n) {
        let chosen: Vec<&PositionPath> = choice.iter().map(|&i| &paths[i]).collect();
        if !pairwise_non_overlapping(&chosen) {
            continue;
        }
        let max_depth = chosen.iter().map(|p| p.path.len()).max().unwrap_or(0);
        let mut subs = Vec::with_capacity(n);
        let pattern = substitute_wildcards(ty, &chosen, &mut subs);
        // `subs` is filled by substitute_wildcards in document order of where
        // the wildcards appear in the pattern.
        variants.push((max_depth, pattern, subs));
    }

    // Sort by (max_depth desc) then by stable original order.
    variants.sort_by(|a, b| b.0.cmp(&a.0));
    variants.into_iter().map(|(_, p, s)| (p, s)).collect()
}

/// Path from the root of a `syn::Type` to one specific subtype position.
/// Represented as a sequence of child-indices into `immediate_subtype_positions`.
#[derive(Clone, Debug)]
struct PositionPath {
    path: Vec<usize>,
}

fn collect_positions(ty: &syn::Type, prefix: &mut Vec<usize>, out: &mut Vec<PositionPath>) {
    let positions = positions_for_traversal(ty);
    for (i, sub) in positions.iter().enumerate() {
        prefix.push(i);
        out.push(PositionPath { path: prefix.clone() });
        collect_positions(sub, prefix, out);
        prefix.pop();
    }
}

/// Same as `immediate_subtype_positions` but for `impl Fn(args)` returns
/// the args (since we substitute at that level too).
fn positions_for_traversal(ty: &syn::Type) -> Vec<syn::Type> {
    if let Some(args) = crate::core::registry::extract_fn_trait_args(ty) {
        return args;
    }
    immediate_subtype_positions(ty)
}

/// True iff none of `paths` is a strict prefix of another. Equal paths
/// trivially overlap (we don't generate equal paths anyway).
fn pairwise_non_overlapping(paths: &[&PositionPath]) -> bool {
    for i in 0..paths.len() {
        for j in (i + 1)..paths.len() {
            if is_prefix(&paths[i].path, &paths[j].path) || is_prefix(&paths[j].path, &paths[i].path)
            {
                return false;
            }
        }
    }
    true
}

fn is_prefix(short: &[usize], long: &[usize]) -> bool {
    if short.len() > long.len() {
        return false;
    }
    short.iter().zip(long.iter()).all(|(a, b)| a == b)
}

/// Iterate every size-`k` subset of `0..n` as `Vec<usize>` in lex order.
fn choose_indices(n: usize, k: usize) -> Vec<Vec<usize>> {
    if k == 0 || k > n {
        return vec![];
    }
    let mut out = Vec::new();
    let mut current: Vec<usize> = (0..k).collect();
    loop {
        out.push(current.clone());
        // Find the rightmost element that can be incremented.
        let mut i = k;
        while i > 0 {
            i -= 1;
            if current[i] < n - (k - i) {
                current[i] += 1;
                for j in (i + 1)..k {
                    current[j] = current[j - 1] + 1;
                }
                break;
            }
            if i == 0 {
                return out;
            }
        }
        if current[0] > n - k {
            break;
        }
    }
    out
}

/// Build the pattern by walking `ty` and replacing each chosen position
/// with `_`. Subtypes at the chosen positions are pushed into `subs` in
/// the document order of where the wildcards appear in the pattern.
fn substitute_wildcards(
    ty: &syn::Type,
    chosen: &[&PositionPath],
    subs: &mut Vec<syn::Type>,
) -> syn::Type {
    let mut prefix = Vec::new();
    walk_substitute(ty, &mut prefix, chosen, subs)
}

fn walk_substitute(
    ty: &syn::Type,
    prefix: &mut Vec<usize>,
    chosen: &[&PositionPath],
    subs: &mut Vec<syn::Type>,
) -> syn::Type {
    let positions = positions_for_traversal(ty);
    if positions.is_empty() {
        return ty.clone();
    }
    let mut new_subs: Vec<syn::Type> = Vec::with_capacity(positions.len());
    for (i, sub) in positions.iter().enumerate() {
        prefix.push(i);
        let is_chosen = chosen.iter().any(|p| p.path == *prefix);
        if is_chosen {
            subs.push(sub.clone());
            new_subs.push(syn::parse_quote!(_));
        } else {
            new_subs.push(walk_substitute(sub, prefix, chosen, subs));
        }
        prefix.pop();
    }
    rebuild_type_with_positions(ty, &new_subs)
}

/// Rebuild a type by replacing its immediate child positions with `new_subs`.
fn rebuild_type_with_positions(ty: &syn::Type, new_subs: &[syn::Type]) -> syn::Type {
    if let Some(_args) = crate::core::registry::extract_fn_trait_args(ty) {
        // Reconstruct `impl Fn(new_subs[0], new_subs[1], ...) + Send + Sync + 'static`.
        let args = new_subs;
        let tokens = quote::quote!(impl Fn(#(#args),*) + Send + Sync + 'static);
        return syn::parse2(tokens).expect("rebuild impl Fn must parse");
    }
    match ty {
        syn::Type::Path(p) => {
            let mut new = p.clone();
            if let Some(last) = new.path.segments.last_mut() {
                if let syn::PathArguments::AngleBracketed(ab) = &mut last.arguments {
                    let mut idx = 0;
                    for arg in ab.args.iter_mut() {
                        if let syn::GenericArgument::Type(t) = arg {
                            *t = new_subs[idx].clone();
                            idx += 1;
                        }
                    }
                }
            }
            syn::Type::Path(new)
        }
        syn::Type::Reference(r) => {
            let mut new = r.clone();
            *new.elem = new_subs[0].clone();
            syn::Type::Reference(new)
        }
        syn::Type::Tuple(t) => {
            let mut new = t.clone();
            new.elems.clear();
            for s in new_subs {
                new.elems.push(s.clone());
            }
            syn::Type::Tuple(new)
        }
        syn::Type::Array(a) => {
            let mut new = a.clone();
            *new.elem = new_subs[0].clone();
            syn::Type::Array(new)
        }
        syn::Type::Slice(s) => {
            let mut new = s.clone();
            *new.elem = new_subs[0].clone();
            syn::Type::Slice(new)
        }
        syn::Type::Ptr(p) => {
            let mut new = p.clone();
            *new.elem = new_subs[0].clone();
            syn::Type::Ptr(new)
        }
        syn::Type::Group(g) => {
            let mut new = g.clone();
            *new.elem = rebuild_type_with_positions(&g.elem, new_subs);
            syn::Type::Group(new)
        }
        syn::Type::Paren(p) => {
            let mut new = p.clone();
            *new.elem = rebuild_type_with_positions(&p.elem, new_subs);
            syn::Type::Paren(new)
        }
        other => other.clone(),
    }
}

// ──────────────────────────────────────────────────────────────────────
// Required-flag propagation (BFS from required entries through `subs`)
// ──────────────────────────────────────────────────────────────────────

fn propagate_required(registry: &mut Registry) {
    // Seed the queue from scan-time required keys plus any `required: true`
    // already on resolved entries.
    let mut queue: VecDeque<(Direction, TypeKey)> = VecDeque::new();
    for k in &registry.required_inputs_scan {
        queue.push_back((Direction::Input, k.clone()));
    }
    for k in &registry.required_outputs_scan {
        queue.push_back((Direction::Output, k.clone()));
    }

    while let Some((dir, key)) = queue.pop_front() {
        // Mark this entry's `required: true` if it's resolved.
        let subs = mark_and_get_subs(registry, dir, &key);
        // Subs travel in the same direction as the parent — they're the
        // inner converters this body delegates to.
        for sub_key in subs {
            if !is_required_resolved(registry, dir, &sub_key) {
                set_required(registry, dir, &sub_key);
                queue.push_back((dir, sub_key));
            }
        }
    }
}

fn mark_and_get_subs(registry: &mut Registry, dir: Direction, key: &TypeKey) -> Vec<TypeKey> {
    let buckets = match dir {
        Direction::Input => &mut registry.input_types,
        Direction::Output => &mut registry.output_types,
    };
    for bucket in buckets.iter_mut() {
        if let Some(slot) = bucket.get_mut(key) {
            if let Some(entry) = slot {
                entry.required = true;
                return entry.subs.clone();
            }
            return vec![];
        }
    }
    vec![]
}

fn is_required_resolved(registry: &Registry, dir: Direction, key: &TypeKey) -> bool {
    let buckets = match dir {
        Direction::Input => &registry.input_types,
        Direction::Output => &registry.output_types,
    };
    for bucket in buckets {
        if let Some(slot) = bucket.get(key) {
            return slot.as_ref().is_some_and(|e| e.required);
        }
    }
    false
}

fn set_required(registry: &mut Registry, dir: Direction, key: &TypeKey) {
    match dir {
        Direction::Input => {
            registry.required_inputs_scan.insert(key.clone());
        }
        Direction::Output => {
            registry.required_outputs_scan.insert(key.clone());
        }
    }
    let buckets = match dir {
        Direction::Input => &mut registry.input_types,
        Direction::Output => &mut registry.output_types,
    };
    for bucket in buckets.iter_mut() {
        if let Some(Some(entry)) = bucket.get_mut(key) {
            entry.required = true;
            return;
        }
    }
}

fn final_invariant_check(registry: &Registry) -> Result<(), ResolveError> {
    let mut entries: Vec<UnresolvedEntry> = Vec::new();
    let scan_required_input = &registry.required_inputs_scan;
    let scan_required_output = &registry.required_outputs_scan;

    for (i, bucket) in registry.input_types.iter().enumerate() {
        let _ = i;
        for (key, slot) in bucket {
            let needs = match slot {
                Some(e) => e.required,
                None => scan_required_input.contains(key),
            };
            if needs && slot.is_none() {
                entries.push(UnresolvedEntry {
                    key: key.clone(),
                    direction: Direction::Input,
                    location: registry.type_locations.get(key).cloned(),
                });
            }
        }
    }
    for (i, bucket) in registry.output_types.iter().enumerate() {
        let _ = i;
        for (key, slot) in bucket {
            let needs = match slot {
                Some(e) => e.required,
                None => scan_required_output.contains(key),
            };
            if needs && slot.is_none() {
                entries.push(UnresolvedEntry {
                    key: key.clone(),
                    direction: Direction::Output,
                    location: registry.type_locations.get(key).cloned(),
                });
            }
        }
    }
    if entries.is_empty() {
        Ok(())
    } else {
        Err(ResolveError::Unresolved { entries })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::ToTokens;

    fn ty(s: &str) -> syn::Type {
        syn::parse_str(s).unwrap()
    }

    fn variant_strs(v: &[(syn::Type, Vec<syn::Type>)]) -> Vec<(String, Vec<String>)> {
        v.iter()
            .map(|(p, s)| {
                (
                    p.to_token_stream().to_string(),
                    s.iter().map(|t| t.to_token_stream().to_string()).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn rank_1_variants_for_result_option_string() {
        let t = ty("Result<Option<u64>, String>");
        let v = enumerate_wildcard_subs(&t, 1);
        let s = variant_strs(&v);
        // Three rank-1 variants. Deepest-first: u64-substitution comes
        // before Option<u64>-substitution and String-substitution (both depth 1).
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].0, "Result < Option < _ > , String >");
        assert_eq!(s[0].1, vec!["u64"]);
    }

    #[test]
    fn rank_2_variants_for_result_option_string() {
        let t = ty("Result<Option<u64>, String>");
        let v = enumerate_wildcard_subs(&t, 2);
        let s = variant_strs(&v);
        // Two rank-2 variants. Deepest-first: (u64, String) before (Option<u64>, String).
        assert_eq!(s.len(), 2);
        assert!(
            s[0].0.contains("Option < _ >") && s[0].0.contains(", _"),
            "expected Result<Option<_>, _>, got {}",
            s[0].0
        );
    }

    #[test]
    fn rank_3_zero_variants_for_rank_2_type() {
        let t = ty("Result<Option<u64>, String>");
        assert!(enumerate_wildcard_subs(&t, 3).is_empty());
    }

    #[test]
    fn rank_1_for_vec_option_u64_deepest_first() {
        let t = ty("Vec<Option<u64>>");
        let v = enumerate_wildcard_subs(&t, 1);
        let s = variant_strs(&v);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].0, "Vec < Option < _ > >");
        assert_eq!(s[0].1, vec!["u64"]);
        assert_eq!(s[1].0, "Vec < _ >");
        assert_eq!(s[1].1, vec!["Option < u64 >"]);
    }

    #[test]
    fn impl_fn_decomposition() {
        let t = ty("impl Fn(u64, String) + Send + Sync + 'static");
        let v = enumerate_wildcard_subs(&t, 2);
        let s = variant_strs(&v);
        assert_eq!(s.len(), 1);
        assert!(s[0].0.contains("impl Fn (_ , _)"), "got {}", s[0].0);
    }

    #[test]
    fn rank_0_no_variants() {
        let t = ty("u64");
        assert!(enumerate_wildcard_subs(&t, 1).is_empty());
    }
}
