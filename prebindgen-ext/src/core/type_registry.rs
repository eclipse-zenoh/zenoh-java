//! Universal collection of [`TypeBinding`]s, keyed by the canonical
//! `to_token_stream()` form of the Rust type-shape.

use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::quote;

use crate::core::inline_fn::InlineFn;
use crate::core::type_binding::{canon_type, TypeBinding};

#[derive(Default, Clone)]
pub struct TypeRegistry {
    pub(crate) types: HashMap<String, TypeBinding>,
}

impl TypeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add (or replace) a [`TypeBinding`] in this registry.
    pub fn type_binding(mut self, binding: TypeBinding) -> Self {
        self.types.insert(binding.name().to_string(), binding);
        self
    }

    /// Look up a registered [`TypeBinding`] by its canonical type-shape key
    /// (e.g. `"HistoryConfig"`, `"Vec < u8 >"`). The key is canonicalized
    /// via `syn::Type` parse so callers can pass either spacing form.
    pub fn type_by_key(&self, key: &str) -> Option<&TypeBinding> {
        self.types.get(&canon_type(key))
    }

    /// Merge another registry into this one. Entries in `other` override
    /// entries with the same key in `self`.
    pub fn merge(mut self, other: TypeRegistry) -> Self {
        self.types.extend(other.types);
        self
    }

    /// Internal: insert a raw entry without going through the canonicaliser
    /// (used by `StructStrategy` impls that compute the key themselves).
    pub(crate) fn insert_raw(&mut self, key: String, binding: TypeBinding) {
        self.types.insert(key, binding);
    }

    /// Internal: drain the entries of another registry into this one.
    /// Used by builder fluent methods that take `TypeRegistry` by value.
    pub(crate) fn extend_from(&mut self, other: TypeRegistry) {
        self.types.extend(other.types);
    }
}

/// Pre-built registry containing universal language-primitive rows
/// (`bool`, `i64`, `f64`). These have JNI-shaped wire forms today; if a
/// non-JNI destination is added, callers should construct their own
/// builtins set rather than relying on this one.
///
/// Kept here as a free function so the universal core has no opinion
/// about which primitives are pre-registered.
pub fn primitive_builtins() -> TypeRegistry {
    let bool_row = TypeBinding::input(
        "bool",
        "jni::sys::jboolean",
        InlineFn::new(|input: Option<&syn::Ident>| -> TokenStream {
            let input = input.expect("bool decode requires an input ident");
            quote! { #input != 0 }
        }),
    );

    let i64_row = TypeBinding::input(
        "i64",
        "jni::sys::jlong",
        InlineFn::new(|input: Option<&syn::Ident>| -> TokenStream {
            let input = input.expect("i64 decode requires an input ident");
            quote! { #input }
        }),
    );

    let f64_row = TypeBinding::input(
        "f64",
        "jni::sys::jdouble",
        InlineFn::new(|input: Option<&syn::Ident>| -> TokenStream {
            let input = input.expect("f64 decode requires an input ident");
            quote! { #input }
        }),
    );

    let duration_row = TypeBinding::input(
        "Duration",
        "jni::sys::jlong",
        InlineFn::new(|input: Option<&syn::Ident>| -> TokenStream {
            let input = input.expect("Duration decode requires an input ident");
            quote! { std::time::Duration::from_millis(#input as u64) }
        }),
    );

    TypeRegistry::new()
        .type_binding(bool_row)
        .type_binding(i64_row)
        .type_binding(f64_row)
        .type_binding(duration_row)
}
