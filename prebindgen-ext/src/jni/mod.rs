//! JNI back-end for the Registry pipeline.
//!
//! [`JniExt`] implements [`crate::core::prebindgen_ext::PrebindgenExt`]
//! (Rust-side conversion bodies) and provides an inherent
//! [`JniExt::write_kotlin`] for emitting all Kotlin output (per-callback
//! fun-interface files, `NativeHandle.kt`, typed-handle classes,
//! `JNIWrappers.kt`).

pub mod byte_array_helpers;
pub mod jni_binding_error;
pub mod jni_ext;
pub(crate) mod jni_kotlin_ext;
pub mod string_helpers;
pub(crate) mod wire_access;

pub use byte_array_helpers::{decode_byte_array, encode_byte_array, null_byte_array};
pub use jni_binding_error::JniBindingError;
pub use jni_ext::JniExt;
pub use string_helpers::{decode_string, encode_string, null_string};
