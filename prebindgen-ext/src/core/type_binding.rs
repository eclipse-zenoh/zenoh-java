//! Per-type registry row describing how a Rust type-shape is carried
//! across the FFI boundary.
//!
//! Each binding is keyed by the canonical `to_token_stream()` form of the
//! Rust type-shape (e.g. `"String"`, `"& Session"`, `"Vec < u8 >"`,
//! `"Option < KeyExpr < 'static > >"`, `"ZResult < ZenohId >"`).
//!
//! Wrapper types (`&T`, `Vec<T>`, `Option<T>`, `ZResult<T>`) are **not**
//! decomposed automatically — each must have its own explicit row.
//! Destination-language convenience builders (e.g. `jni::opaque::*`) help
//! keep registration concise.

use proc_macro2::TokenStream;
use quote::ToTokens;

use crate::core::inline_fn::{InputFn, OutputFn};

/// Per-row binding from a Rust type-shape to its FFI wire form.
#[derive(Clone)]
pub(crate) struct TypeBinding {
    pub(crate) rust_type: syn::Type,
    pub(crate) wire_type: syn::Type,
    pub(crate) decode: Option<InputFn>,
    pub(crate) encode: Option<OutputFn>,
}

impl TypeBinding {
    /// Construct a new binding from raw parts.
    pub(crate) fn input_output(
        rust_type: syn::Type,
        wire_type: syn::Type,
        decode: Option<InputFn>,
        encode: Option<OutputFn>,
    ) -> Self {
        Self {
            rust_type,
            wire_type,
            decode,
            encode,
        }
    }

    pub(crate) fn wire_type(&self) -> &syn::Type {
        &self.wire_type
    }
    /// Crate-public accessor for `wire_type` used by sibling modules
    /// (e.g. the Kotlin generator) that need to inspect wire shape.
    pub(crate) fn wire_type_ref(&self) -> &syn::Type {
        &self.wire_type
    }
    pub(crate) fn decode(&self) -> Option<&InputFn> {
        self.decode.as_ref()
    }
    pub(crate) fn encode(&self) -> Option<&OutputFn> {
        self.encode.as_ref()
    }

    /// `&T` row — the wrapped fn receives `&name` instead of `name`.
    pub(crate) fn is_borrow(&self) -> bool {
        matches!(self.rust_type, syn::Type::Reference(_))
    }
    /// `*const _` / `*mut _` wire type — Rust pat ident gets a `_ptr` suffix.
    pub(crate) fn is_pointer(&self) -> bool {
        matches!(self.wire_type, syn::Type::Ptr(_))
    }
    /// Apply `f` to this binding's `decode` to produce a `TokenStream` for
    /// the given input ident. Used by `FunctionsConverter` to build the
    /// per-arg prelude.
    pub(crate) fn call_decode(&self, input: &syn::Ident) -> Option<TokenStream> {
        self.decode.as_ref().map(|d| d.call(input))
    }
}

/// Canonical type-shape string. Parses through `syn::Type` so whitespace
/// variations in user input (`"Vec<u8>"` vs `"Vec < u8 >"`) match the form
/// the classifier produces from AST nodes via `to_token_stream()`.
pub(crate) fn canon_type(s: &str) -> String {
    syn::parse_str::<syn::Type>(s)
        .map(|t| t.to_token_stream().to_string())
        .unwrap_or_else(|e| panic!("TypeBinding: cannot parse `{}` as a type: {}", s, e))
}
