//! Reusable collection of JNI type bindings.
//!
//! [`JniTypeBinding`] aggregates a set of [`TypeBinding`]s — including
//! callback registrations, which live as `consume` slots on the element
//! type's binding — into a single value that can be defined once and
//! threaded through both phases of the JNI binding pipeline (a
//! [`crate::jni_converter::JniStructConverter`] mutates it; a
//! [`crate::jni_converter::JniMethodsConverter`] reads it).
//!
//! ```ignore
//! use zenoh_flat::jni_converter::{InlineFn, JniForm, TypeBinding};
//! use zenoh_flat::jni_type_binding::JniTypeBinding;
//! use quote::quote;
//!
//! let common = JniTypeBinding::new()
//!     .type_binding(
//!         TypeBinding::new("KeyExpr").consume(
//!             JniForm::new(
//!                 "*const zenoh::key_expr::KeyExpr<'static>",
//!                 "Long",
//!                 InlineFn::new(|input| {
//!                     quote! { (*std::sync::Arc::from_raw(#input)).clone() }
//!                 }),
//!             )
//!             .pointer_param(true),
//!         ),
//!     );
//! ```

use std::collections::HashMap;

use quote::{quote, ToTokens};

use crate::jni_converter::{InlineFn, JniForm, ReturnForm};

/// Per-type description of how a Rust type is represented across the JNI
/// boundary. A type may declare up to four forms:
/// `consume` (`T` parameter), `borrow` (`&T` parameter), `returns`
/// (`ZResult<T>` return), and `returns_vec` (`ZResult<Vec<T>>` return).
///
/// Callback parameters (`impl Fn(T) + Send + Sync + 'static`) are described
/// by an ordinary `consume` form on a binding keyed under
/// `"impl Fn(<element>)"` (e.g. `"impl Fn(Sample)"`, `"impl Fn()"`). The
/// classifier synthesizes that key when it sees an `impl Fn(...)` parameter.
#[derive(Clone)]
pub struct TypeBinding {
    pub(crate) name: String,
    /// Kotlin-side type name (FQN preferred — out-of-package import is
    /// auto-derived; bare for same-package). Used as the Kotlin parameter
    /// type when the form's wire JNI type is `JObject` and as the Kotlin
    /// return type. For primitive-mapped forms (`bool`, `Duration`,
    /// `String`, ...) the form's `kotlin_jni_type` is used instead.
    pub(crate) kotlin_type: Option<String>,
    pub(crate) consume: Option<JniForm>,
    pub(crate) borrow: Option<JniForm>,
    pub(crate) returns: Option<ReturnForm>,
    pub(crate) returns_vec: Option<ReturnForm>,
    /// Decoder path for Java-enum-shaped types (`fn(jint) -> ZResult<T>`).
    /// Set when the type's JNI representation is an `Int` mapped through a
    /// pure decoder. Used by struct-field classification to detect enum
    /// fields and emit `env.get_field(..., "I")` + decoder call.
    pub(crate) enum_decoder: Option<syn::Path>,
}

impl TypeBinding {
    /// Short type name this binding is keyed under (e.g. `"KeyExpr"`).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Construct a binding keyed by `name`. If `name` parses as a Rust type,
    /// it is canonicalized through `quote::ToTokens` so whitespace variations
    /// in user input match the form the classifier produces from AST nodes
    /// (matters for `impl Fn(T) + Send + Sync + 'static`-style names). Falls
    /// back to the literal string if parsing fails.
    pub fn new(name: impl Into<String>) -> Self {
        let raw = name.into();
        let canonical = syn::parse_str::<syn::Type>(&raw)
            .map(|t| t.to_token_stream().to_string())
            .unwrap_or_else(|_| raw);
        Self {
            name: canonical,
            kotlin_type: None,
            consume: None,
            borrow: None,
            returns: None,
            returns_vec: None,
            enum_decoder: None,
        }
    }

    pub fn kotlin(mut self, fqn: impl Into<String>) -> Self {
        self.kotlin_type = Some(fqn.into());
        self
    }

    /// Mark this binding as a Java-enum-shaped type and record the
    /// `fn(jint) -> ZResult<T>` decoder path used for both top-level
    /// argument decoding and struct-field decoding.
    pub fn enum_decoder(mut self, path: impl AsRef<str>) -> Self {
        self.enum_decoder = Some(
            syn::parse_str(path.as_ref()).expect("invalid TypeBinding::enum_decoder path"),
        );
        self
    }

    pub fn consume(mut self, form: JniForm) -> Self {
        self.consume = Some(form);
        self
    }

    pub fn borrow(mut self, form: JniForm) -> Self {
        self.borrow = Some(form);
        self
    }

    pub fn returns(mut self, form: ReturnForm) -> Self {
        self.returns = Some(form);
        self
    }

    pub fn returns_vec(mut self, form: ReturnForm) -> Self {
        self.returns_vec = Some(form);
        self
    }
}

/// Reusable collection of [`TypeBinding`]s plus the Kotlin `data class`
/// strings produced by struct processing.
///
/// The same value flows through both pipeline phases: the
/// [`crate::jni_converter::JniStructConverter`] inserts an auto-generated
/// `TypeBinding` plus a `data class` block for each `#[prebindgen]` struct
/// it sees; the [`crate::jni_converter::JniMethodsConverter`] then reads the
/// type registry to classify args/returns and reads the data-class strings
/// when emitting the final Kotlin file.
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
        self.types.insert(binding.name().to_string(), binding);
        self
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
    /// without any project-specific decoder path: `bool` (inline `x != 0`)
    /// and `Duration` (inline `Duration::from_millis(x as u64)`).
    ///
    /// Types whose decoder lives outside this crate — `String`, `Vec<u8>`,
    /// callbacks, enums, opaque handles — are registered by the caller via
    /// the universal [`JniTypeBinding::type_binding`] entry point. `Vec<u8>`
    /// is keyed under the synthetic name `"VecU8"` (looked up explicitly by
    /// the methods-phase classifier when it sees `Vec<u8>`).
    pub fn with_builtins(mut self) -> Self {
        // bool — jboolean, inline `x != 0`.
        self.types.insert(
            "bool".to_string(),
            TypeBinding::new("bool").consume(JniForm::new(
                "jni::sys::jboolean",
                "Boolean",
                InlineFn::new(|input| quote! { #input != 0 }),
            )),
        );
        // Duration — jlong, inline `Duration::from_millis(x as u64)`.
        self.types.insert(
            "Duration".to_string(),
            TypeBinding::new("Duration").consume(JniForm::new(
                "jni::sys::jlong",
                "Long",
                InlineFn::new(|input| {
                    quote! { std::time::Duration::from_millis(#input as u64) }
                }),
            )),
        );
        self
    }
}
