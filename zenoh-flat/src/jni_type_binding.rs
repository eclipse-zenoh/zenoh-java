//! Flat JNI type registry.
//!
//! Each [`TypeBinding`] is a single row keyed by the canonical
//! `to_token_stream()` form of a Rust type-shape, e.g. `"String"`,
//! `"& Session"`, `"Vec < u8 >"`, `"Option < KeyExpr < 'static > >"`,
//! `"ZResult < ZenohId >"`, `"impl Fn (Sample) + Send + Sync + 'static"`.
//!
//! A row carries:
//!
//! * `kotlin_type` — the Kotlin parameter or return type
//! * `jni_type`    — the on-the-wire JNI type emitted in the wrapper signature
//! * `decode`      — JNI value → Rust value (param-direction rows)
//! * `encode` + `default_expr` — Rust value → JNI value (return-direction rows)
//!
//! Wrapper types (`&T`, `Vec<T>`, `Option<T>`, `ZResult<T>`) are **not**
//! decomposed by the classifier — each must have its own explicit row. The
//! [`TypeBinding::opaque_borrow`], [`TypeBinding::opaque_arc_return`] and
//! [`TypeBinding::option_of`] convenience constructors keep registration
//! concise.

use std::collections::HashMap;
use std::sync::Arc;

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};

/// Clonable closure that produces a `TokenStream` from the JNI input ident.
#[derive(Clone)]
pub struct InlineFn(Arc<dyn Fn(&syn::Ident) -> TokenStream + Send + Sync>);

impl InlineFn {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&syn::Ident) -> TokenStream + Send + Sync + 'static,
    {
        InlineFn(Arc::new(f))
    }

    /// `<path>(<input>)?` — pure conversion (e.g. enum decoders).
    pub fn pure(path: impl AsRef<str>) -> Self {
        let s = path.as_ref().to_string();
        InlineFn::new(move |input| {
            let p: syn::Path = syn::parse_str(&s).expect("invalid InlineFn::pure path");
            quote! { #p(#input)? }
        })
    }

    /// `<path>(&env, &<input>)?` — decoder needing shared access to the JNI env.
    pub fn env_ref(path: impl AsRef<str>) -> Self {
        let s = path.as_ref().to_string();
        InlineFn::new(move |input| {
            let p: syn::Path = syn::parse_str(&s).expect("invalid InlineFn::env_ref path");
            quote! { #p(&env, &#input)? }
        })
    }

    /// `<path>(&mut env, &<input>)?` — decoder needing mutable access to the JNI env.
    pub fn env_ref_mut(path: impl AsRef<str>) -> Self {
        let s = path.as_ref().to_string();
        InlineFn::new(move |input| {
            let p: syn::Path =
                syn::parse_str(&s).expect("invalid InlineFn::env_ref_mut path");
            quote! { #p(&mut env, &#input)? }
        })
    }

    pub(crate) fn call(&self, ident: &syn::Ident) -> TokenStream {
        (self.0)(ident)
    }
}

/// How a Rust return value is encoded into a JNI return.
#[derive(Clone)]
pub enum ReturnEncode {
    /// `<path>(&mut env, __result)` — wrapping function returns
    /// `ZResult<jni_type>`.
    Wrapper(syn::Path),
    /// `Ok(Arc::into_raw(Arc::new(__result)))` — opaque Arc-handle return.
    ArcIntoRaw,
}

impl ReturnEncode {
    pub fn wrapper(path: impl AsRef<str>) -> Self {
        ReturnEncode::Wrapper(
            syn::parse_str(path.as_ref()).expect("invalid ReturnEncode::wrapper path"),
        )
    }
}

/// Per-row binding from a Rust type-shape to its JNI/Kotlin representation.
#[derive(Clone)]
pub struct TypeBinding {
    /// Canonical Rust type-shape (token-stream form). The lookup key.
    pub(crate) rust_type: String,
    /// Kotlin parameter / return type (FQN preferred for object types,
    /// bare for primitives). For Option-shaped rows this is the inner
    /// type's Kotlin name; the `?` suffix is added by the emitter from
    /// the row's rust_type prefix.
    pub(crate) kotlin_type: String,
    /// On-the-wire JNI type emitted in the wrapper signature.
    pub(crate) jni_type: syn::Type,
    /// JNI value → Rust value. None for return-only rows.
    pub(crate) decode: Option<InlineFn>,
    /// Rust value → JNI value. None for param-only rows.
    pub(crate) encode: Option<ReturnEncode>,
    /// Default JNI value emitted on the throw-return path. Required when
    /// `encode` is set.
    pub(crate) default_expr: Option<syn::Expr>,
}

impl TypeBinding {
    /// Param-direction row. `rust_type` is canonicalized via `syn::Type` parse.
    pub fn param(
        rust_type: impl AsRef<str>,
        kotlin_type: impl Into<String>,
        jni_type: impl AsRef<str>,
        decode: InlineFn,
    ) -> Self {
        Self {
            rust_type: canon_type(rust_type.as_ref()),
            kotlin_type: kotlin_type.into(),
            jni_type: parse_type(jni_type.as_ref()),
            decode: Some(decode),
            encode: None,
            default_expr: None,
        }
    }

    /// Return-direction row.
    pub fn returns(
        rust_type: impl AsRef<str>,
        kotlin_type: impl Into<String>,
        jni_type: impl AsRef<str>,
        encode: ReturnEncode,
        default_expr: impl AsRef<str>,
    ) -> Self {
        Self {
            rust_type: canon_type(rust_type.as_ref()),
            kotlin_type: kotlin_type.into(),
            jni_type: parse_type(jni_type.as_ref()),
            decode: None,
            encode: Some(encode),
            default_expr: Some(
                syn::parse_str(default_expr.as_ref())
                    .expect("invalid TypeBinding::returns default_expr"),
            ),
        }
    }

    /// Convenience: opaque borrow `&T` — JNI side passes raw `*const T`,
    /// decoded via `<owned_object>::from_raw`. Because the row's key starts
    /// with `&`, the wrapped fn receives `&name` automatically.
    pub fn opaque_borrow(t: impl AsRef<str>, owned_object: impl AsRef<str>) -> Self {
        let t = t.as_ref().to_string();
        let owned_str = owned_object.as_ref().to_string();
        // Validate the owner path parses now so errors surface at registration.
        let _: syn::Path =
            syn::parse_str(&owned_str).expect("opaque_borrow: invalid owned_object path");
        Self::param(
            format!("&{}", t),
            "Long",
            format!("*const {}", t),
            InlineFn::new(move |input| {
                let owned: syn::Path =
                    syn::parse_str(&owned_str).expect("owned_object must parse");
                quote! { #owned::from_raw(#input) }
            }),
        )
    }

    /// Convenience: opaque Arc return for `ZResult<T>` — encode via
    /// `Arc::into_raw(Arc::new(__result))`, default to `std::ptr::null()`.
    pub fn opaque_arc_return(t: impl AsRef<str>) -> Self {
        let t = t.as_ref();
        Self::returns(
            format!("ZResult<{}>", t),
            "Long",
            format!("*const {}", t),
            ReturnEncode::ArcIntoRaw,
            "std::ptr::null()",
        )
    }

    /// Convenience: `Option<X>` row that lifts `inner`'s decode with a
    /// JNI-side null check. Inner's wire type must be JNI-object-shaped.
    pub fn option_of(inner: &TypeBinding) -> Self {
        let inner_decode = inner
            .decode
            .as_ref()
            .expect("option_of: inner must be a param row")
            .clone();
        assert!(
            jni_object_shaped(&inner.jni_type),
            "option_of requires a JNI-object inner form, got `{}`",
            inner.jni_type.to_token_stream()
        );
        Self {
            rust_type: canon_type(&format!("Option<{}>", inner.rust_type)),
            kotlin_type: inner.kotlin_type.clone(),
            jni_type: inner.jni_type.clone(),
            decode: Some(InlineFn::new(move |input| {
                let inner_expr = inner_decode.call(input);
                quote! {
                    if !#input.is_null() {
                        Some(#inner_expr)
                    } else {
                        None
                    }
                }
            })),
            encode: None,
            default_expr: None,
        }
    }

    /// Canonical type-shape this binding is keyed under.
    pub fn name(&self) -> &str {
        &self.rust_type
    }

    pub(crate) fn jni_type(&self) -> &syn::Type {
        &self.jni_type
    }
    pub(crate) fn kotlin_type(&self) -> &str {
        &self.kotlin_type
    }
    pub(crate) fn decode(&self) -> Option<&InlineFn> {
        self.decode.as_ref()
    }
    pub(crate) fn encode(&self) -> Option<&ReturnEncode> {
        self.encode.as_ref()
    }
    pub(crate) fn default_expr(&self) -> Option<&syn::Expr> {
        self.default_expr.as_ref()
    }
    /// `&T` row — wrapped fn receives `&name`.
    pub(crate) fn is_borrow(&self) -> bool {
        self.rust_type.starts_with('&')
    }
    /// `*const _` / `*mut _` wire type — Kotlin name gets `Ptr` suffix and
    /// the Rust ident gets `_ptr` suffix.
    pub(crate) fn is_pointer(&self) -> bool {
        matches!(self.jni_type, syn::Type::Ptr(_))
    }
    /// `Option<_>` row — Kotlin emission appends `?`.
    pub(crate) fn is_option(&self) -> bool {
        self.rust_type.starts_with("Option <")
    }
}

/// Reusable collection of [`TypeBinding`]s plus the Kotlin `data class`
/// strings produced by struct processing.
#[derive(Default, Clone)]
pub struct JniTypeBinding {
    pub(crate) types: HashMap<String, TypeBinding>,
    pub(crate) kotlin_data_classes: Vec<String>,
}

impl JniTypeBinding {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add (or replace) a [`TypeBinding`] in this collection.
    pub fn type_binding(mut self, binding: TypeBinding) -> Self {
        self.types.insert(binding.rust_type.clone(), binding);
        self
    }

    /// Look up a registered [`TypeBinding`] by its canonical type-shape key
    /// (e.g. `"HistoryConfig"`, `"Vec < u8 >"`). The key is canonicalized
    /// via `syn::Type` parse so callers can pass either spacing form.
    pub fn type_by_key(&self, key: &str) -> Option<&TypeBinding> {
        self.types.get(&canon_type(key))
    }

    /// Merge another [`JniTypeBinding`] into this one. Type entries in
    /// `other` override entries with the same key in `self`; data-class
    /// blocks are appended in order.
    pub fn merge(mut self, other: JniTypeBinding) -> Self {
        self.types.extend(other.types);
        self.kotlin_data_classes.extend(other.kotlin_data_classes);
        self
    }

    /// Pre-register built-in language types whose JNI form is fully described
    /// without any project-specific decoder path: `bool`, `i64`, `f64`, and
    /// `Duration`.
    pub fn with_builtins(mut self) -> Self {
        let bool_row = TypeBinding::param(
            "bool",
            "Boolean",
            "jni::sys::jboolean",
            InlineFn::new(|input| quote! { #input != 0 }),
        );
        self.types.insert(bool_row.rust_type.clone(), bool_row);

        let i64_row = TypeBinding::param(
            "i64",
            "Long",
            "jni::sys::jlong",
            InlineFn::new(|input| quote! { #input }),
        );
        self.types.insert(i64_row.rust_type.clone(), i64_row);

        let f64_row = TypeBinding::param(
            "f64",
            "Double",
            "jni::sys::jdouble",
            InlineFn::new(|input| quote! { #input }),
        );
        self.types.insert(f64_row.rust_type.clone(), f64_row);

        let duration_row = TypeBinding::param(
            "Duration",
            "Long",
            "jni::sys::jlong",
            InlineFn::new(|input| {
                quote! { std::time::Duration::from_millis(#input as u64) }
            }),
        );
        self.types
            .insert(duration_row.rust_type.clone(), duration_row);
        self
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
    syn::parse_str(s).unwrap_or_else(|e| panic!("invalid JNI wire type `{}`: {}", s, e))
}

/// True if `ty` is a JNI object-shaped wire type that supports `is_null()`.
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
