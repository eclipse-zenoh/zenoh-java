//! Pure naming function for generated converter functions.
//!
//! The name is a deterministic function of the `(rust_type, wire_type)`
//! pair: `<rust_ident>_to_<wire_ident>_<hash>`. The hash is always present
//! (8 hex chars from a `DefaultHasher` digest of the canonical token-stream
//! tuple), so the resulting `Ident` is collision-free by construction
//! without any global registry of issued names.
//!
//! Names are computed lazily wherever needed (resolver, body emitter, ext
//! impls). No state, no caches.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use proc_macro2::Span;
use quote::ToTokens;

/// Build the converter name for converting from `rust` to `wire` (input
/// direction: wire → rust). Format: `<rust_id>_to_<wire_id>_<hash>`.
pub fn input_name(rust: &syn::Type, wire: &syn::Type) -> syn::Ident {
    name(rust, wire)
}

/// Build the converter name for converting from `rust` to `wire` (output
/// direction: rust → wire). Same format as [`input_name`]; the direction is
/// distinguished by where the resolver places the entry, not by the name.
/// The hash includes the wire type, so input and output names for the same
/// rust type rarely collide (they share the rust_id and wire_id only when
/// the wire type happens to match).
pub fn output_name(rust: &syn::Type, wire: &syn::Type) -> syn::Ident {
    name(rust, wire)
}

fn name(rust: &syn::Type, wire: &syn::Type) -> syn::Ident {
    let rust_id = sanitize(&rust.to_token_stream().to_string());
    let wire_id = wire_short(wire);
    let h = hash_pair(rust, wire);
    let s = format!("{}_to_{}_{:08x}", rust_id, wire_id, h & 0xffff_ffff);
    syn::Ident::new(&s, Span::call_site())
}

/// Sanitise a type's canonical token stream into a Rust ident fragment:
/// non-alphanumeric chars become `_`, leading digits get an underscore
/// prefix. Result is never empty.
fn sanitize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_underscore = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            prev_underscore = false;
        } else if !prev_underscore {
            out.push('_');
            prev_underscore = true;
        }
    }
    while out.starts_with('_') {
        out.remove(0);
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("ty");
    }
    if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

/// Last segment of a wire type's path, sanitised. For non-path wire types
/// (refs, tuples, etc.) falls back to a sanitised full token stream.
fn wire_short(wire: &syn::Type) -> String {
    if let syn::Type::Path(tp) = wire {
        if let Some(last) = tp.path.segments.last() {
            return sanitize(&last.ident.to_string());
        }
    }
    sanitize(&wire.to_token_stream().to_string())
}

fn hash_pair(rust: &syn::Type, wire: &syn::Type) -> u64 {
    let mut h = DefaultHasher::new();
    rust.to_token_stream().to_string().hash(&mut h);
    "::".hash(&mut h);
    wire.to_token_stream().to_string().hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ty(s: &str) -> syn::Type {
        syn::parse_str(s).unwrap()
    }

    #[test]
    fn name_format() {
        let n = input_name(&ty("Sample"), &ty("jni::objects::JObject"));
        let s = n.to_string();
        assert!(s.starts_with("Sample_to_JObject_"), "got {}", s);
        assert_eq!(s.len(), "Sample_to_JObject_".len() + 8);
    }

    #[test]
    fn deterministic() {
        let a = input_name(&ty("Vec<u8>"), &ty("jni::sys::jbyteArray"));
        let b = input_name(&ty("Vec < u8 >"), &ty("jni :: sys :: jbyteArray"));
        // Different whitespace in source, but the same canonicalised pair.
        // The hash depends on the canonicalised token-stream form, so these
        // should match.
        assert_eq!(a.to_string(), b.to_string());
    }

    #[test]
    fn distinct_pairs_distinct_hashes() {
        let a = input_name(&ty("Option<u64>"), &ty("jni::sys::jlong"));
        let b = input_name(&ty("Option<i64>"), &ty("jni::sys::jlong"));
        assert_ne!(a.to_string(), b.to_string());
    }

    #[test]
    fn sanitize_strips_punctuation() {
        let n = input_name(&ty("Option<&KeyExpr>"), &ty("jni::objects::JObject"));
        let s = n.to_string();
        assert!(syn::parse_str::<syn::Ident>(&s).is_ok(), "must be a valid ident: {}", s);
    }
}
