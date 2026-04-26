//! Prebindgen JNI extensions for Zenoh.
//!
//! This crate provides JNI binding generators for items marked with `#[prebindgen]`.

pub mod jni_converter;
pub mod jni_type_binding;

pub use jni_converter::{JniStructConverter, JniMethodsConverter};
pub use jni_type_binding::{JniTypeBinding, TypeBinding, InlineFn, ReturnEncode};
