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

use crate::core::inline_fn::InlineFn;

/// Per-row binding from a Rust type-shape to its FFI wire form.
#[derive(Clone)]
pub(crate) struct TypeBinding {
    pub(crate) rust_type: String,
    pub(crate) wire_type: syn::Type,
    pub(crate) decode: Option<InlineFn>,
    pub(crate) encode: Option<InlineFn>,
}

impl TypeBinding {
    /// Param-direction row. `rust_type` is canonicalized via `syn::Type` parse.
    pub(crate) fn input(
        rust_type: impl AsRef<str>,
        wire_type: impl AsRef<str>,
        decode: InlineFn,
    ) -> Self {
        Self {
            rust_type: canon_type(rust_type.as_ref()),
            wire_type: parse_type(wire_type.as_ref()),
            decode: Some(decode),
            encode: None,
        }
    }

    /// Return-direction row.
    pub(crate) fn output(
        rust_type: impl AsRef<str>,
        wire_type: impl AsRef<str>,
        encode: InlineFn,
    ) -> Self {
        Self {
            rust_type: canon_type(rust_type.as_ref()),
            wire_type: parse_type(wire_type.as_ref()),
            decode: None,
            encode: Some(encode),
        }
    }

    /// Construct a new binding from raw parts.
    pub(crate) fn input_output(
        rust_type: impl AsRef<str>,
        wire_type: syn::Type,
        decode: Option<InlineFn>,
        encode: Option<InlineFn>,
    ) -> Self {
        Self {
            rust_type: canon_type(rust_type.as_ref()),
            wire_type,
            decode,
            encode,
        }
    }

    /// Canonical type-shape this binding is keyed under.
    pub(crate) fn name(&self) -> &str {
        &self.rust_type
    }

    pub(crate) fn wire_type(&self) -> &syn::Type {
        &self.wire_type
    }
    /// Crate-public accessor for `wire_type` used by sibling modules
    /// (e.g. the Kotlin generator) that need to inspect wire shape.
    pub(crate) fn wire_type_ref(&self) -> &syn::Type {
        &self.wire_type
    }
    pub(crate) fn decode(&self) -> Option<&InlineFn> {
        self.decode.as_ref()
    }
    pub(crate) fn encode(&self) -> Option<&InlineFn> {
        self.encode.as_ref()
    }

    /// `&T` row — the wrapped fn receives `&name` instead of `name`.
    pub(crate) fn is_borrow(&self) -> bool {
        self.rust_type.starts_with('&')
    }
    /// `*const _` / `*mut _` wire type — Rust pat ident gets a `_ptr` suffix.
    pub(crate) fn is_pointer(&self) -> bool {
        matches!(self.wire_type, syn::Type::Ptr(_))
    }
    /// `Option<_>` row — destination-language emitters may use this to
    /// append a nullability marker.
    pub(crate) fn is_option(&self) -> bool {
        self.rust_type.starts_with("Option <")
    }

    /// Apply `f` to this binding's `decode` to produce a `TokenStream` for
    /// the given input ident. Used by `FunctionsConverter` to build the
    /// per-arg prelude.
    pub(crate) fn call_decode(&self, input: &syn::Ident) -> Option<TokenStream> {
        self.decode.as_ref().map(|d| d.call(Some(input)))
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

fn parse_type(s: &str) -> syn::Type {
    syn::parse_str(s).unwrap_or_else(|e| panic!("invalid wire type `{}`: {}", s, e))
}

/// True if `ty` is a JNI-object-shaped wire type that supports `is_null()`.
/// Lives in `core` so language-flavoured convenience builders can call it
/// without crossing module boundaries.
pub(crate) fn jni_object_shaped(ty: &syn::Type) -> bool {
    let syn::Type::Path(tp) = ty else { return false };
    let Some(last) = tp.path.segments.last() else {
        return false;
    };
    matches!(
        last.ident.to_string().as_str(),
        "JObject" | "JString" | "JByteArray"
    )
}
