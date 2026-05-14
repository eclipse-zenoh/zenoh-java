//! JNI back-end for the Registry pipeline.
//!
//! [`JniExt`] implements both [`crate::core::prebindgen_ext::PrebindgenExt`]
//! (Rust-side conversion bodies) and [`crate::kotlin::KotlinExt`] (per-
//! callback `.kt` file emission).
//!
//! `wire_access` and `jni_type` provide small JNI sig/wire helpers shared
//! between converter assembly and (eventually) Kotlin emission. `jni_type`
//! is currently dormant — kept for potential reuse by the unmigrated
//! `KotlinInterfaceGenerator`.

pub mod byte_array_helpers;
pub mod jni_ext;
pub mod jni_kotlin_ext;
pub mod jni_type;
pub mod string_helpers;
pub(crate) mod wire_access;

pub use byte_array_helpers::{decode_byte_array, encode_byte_array, null_byte_array};
pub use jni_ext::JniExt;
pub use string_helpers::{decode_string, encode_string, null_string};
