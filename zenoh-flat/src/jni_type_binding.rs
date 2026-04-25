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
//!     )
//!     .callback_decoder(
//!         "Sample",
//!         "crate::sample_callback::process_kotlin_sample_callback",
//!         "io.zenoh.jni.callbacks.JNISubscriberCallback",
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

use crate::jni_converter::{CallbackForm, TypeBinding};

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

    /// Register a callback decoder. `element_type_name` is the last path
    /// segment of the callback's argument type (e.g. `"Sample"`); use
    /// `"()"` for zero-arg callbacks. The decoder must have signature
    /// `fn(&mut JNIEnv, JObject) -> ZResult<impl Fn(T) + Send + Sync + 'static>`.
    /// Sugar over [`JniTypeBinding::type_binding`] that fills in the
    /// binding's [`CallbackForm`].
    pub fn callback_decoder(
        self,
        element_type_name: impl Into<String>,
        decoder: impl AsRef<str>,
        kotlin_type: impl Into<String>,
    ) -> Self {
        let name = element_type_name.into();
        let existing = self.types.get(&name).cloned();
        let binding = existing
            .unwrap_or_else(|| TypeBinding::new(name.clone()))
            .callback(CallbackForm::new(decoder, kotlin_type));
        self.type_binding(binding)
    }

    /// Merge another [`JniTypeBinding`] into this one. Entries in `other`
    /// override entries with the same key in `self`.
    pub fn merge(mut self, other: JniTypeBinding) -> Self {
        self.types.extend(other.types);
        self
    }
}
