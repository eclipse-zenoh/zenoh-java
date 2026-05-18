//! Registry-based universal converter pipeline for `#[prebindgen]` items.
//!
//! Pipeline:
//!   1. [`core::Registry::from_source`] scans a `prebindgen::Source`.
//!   2. [`core::Registry::write_rust`] resolves every required type via
//!      a configured [`core::prebindgen_ext::PrebindgenExt`] and writes
//!      the generated Rust bindings file. Language-agnostic — any
//!      [`PrebindgenExt`] back-end (JNI, future C/cbindgen, etc.) is
//!      accepted.
//!   3. Back-end-specific emitters (e.g. [`jni::JniExt::write_kotlin`])
//!      walk the resolved registry to emit secondary artifacts.
//!
//! The [`jni::JniExt`] back-end is the reference language module.

pub mod core;
pub mod jni;
pub mod kotlin;
mod util;
