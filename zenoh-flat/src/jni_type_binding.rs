//! Reusable collection of JNI type bindings.
//!
//! [`JniTypeBinding`] aggregates a set of [`TypeBinding`]s — including
//! callback registrations, which live as [`crate::jni_converter::CallbackForm`]
//! slots on the element type's binding — into a single value that can be
//! defined once and ingested into many [`crate::jni_converter::Builder`]
//! instances. Useful when a project emits several `JniConverter` outputs
//! (e.g. one per JNI class) that share a common vocabulary of types.
//!
//! ```ignore
//! use zenoh_flat::jni_converter::{ArgDecode, JniConverter, JniForm, TypeBinding};
//! use zenoh_flat::jni_type_binding::JniTypeBinding;
//!
//! let common = JniTypeBinding::new()
//!     .type_binding(
//!         TypeBinding::new("KeyExpr").consume(
//!             JniForm::new(
//!                 "*const zenoh::key_expr::KeyExpr<'static>",
//!                 "Long",
//!                 ArgDecode::ConsumeArc,
//!             )
//!             .pointer_param(true),
//!         ),
//!     );
//!
//! let session_converter = JniConverter::builder()
//!     .class_prefix("Java_io_zenoh_jni_JNISessionNative_")
//!     .jni_type_binding(common.clone())
//!     // ...other builder calls...
//!     .build();
//!
//! let publisher_converter = JniConverter::builder()
//!     .class_prefix("Java_io_zenoh_jni_JNIPublisherNative_")
//!     .jni_type_binding(common)
//!     // ...other builder calls...
//!     .build();
//! ```

use std::collections::HashMap;

use quote::ToTokens;
use crate::jni_converter::{JniForm, ReturnForm};

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
        }
    }

    pub fn kotlin(mut self, fqn: impl Into<String>) -> Self {
        self.kotlin_type = Some(fqn.into());
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

/// Reusable collection of [`TypeBinding`]s.
///
/// Built fluently and consumed by
/// [`crate::jni_converter::Builder::jni_type_binding`]. `Clone` so the same
/// set can be ingested into multiple builders.
#[derive(Default, Clone)]
pub struct JniTypeBinding {
    pub(crate) types: HashMap<String, TypeBinding>,
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

    /// Merge another [`JniTypeBinding`] into this one. Entries in `other`
    /// override entries with the same key in `self`.
    pub fn merge(mut self, other: JniTypeBinding) -> Self {
        self.types.extend(other.types);
        self
    }
}
