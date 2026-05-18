//! Core: language-agnostic primitives for the Registry-based pipeline.
//!
//! Public entry point: [`registry::Registry::from_items`] scans a stream
//! of `(syn::Item, SourceLocation)` into a flat type table;
//! [`registry::Registry::write_rust`]
//! resolves every required type using the configured
//! [`prebindgen_ext::PrebindgenExt`] and emits the bindings file. Kotlin
//! emission for the JNI back-end lives in `crate::jni::JniExt::write_kotlin`.

pub mod niches;
pub mod prebindgen_ext;
pub mod registry;
pub(crate) mod resolve;
pub(crate) mod write;

pub use niches::{NicheSlot, Niches};
pub use registry::Registry;
