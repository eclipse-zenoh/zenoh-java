//! Registry-based universal converter pipeline for `#[prebindgen]` items.
//!
//! Pipeline:
//!   1. [`core::registry::Registry::from_source`] scans a `prebindgen::Source`.
//!   2. [`core::resolve::resolve`] drives a [`core::prebindgen_ext::PrebindgenExt`]
//!      until every required type has a converter.
//!   3. [`core::write::write_rust`] emits the bindings file.
//!   4. [`kotlin::write_kotlin`] (optional) emits Kotlin output via a
//!      [`kotlin::KotlinExt`] implementation.
//!
//! The [`jni::JniExt`] back-end implements both traits and is the
//! reference language module.

pub mod core;
pub mod jni;
pub mod kotlin;
mod util;

// Re-exports kept alive transitionally for the unmigrated
// `KotlinInterfaceGenerator`. They will go away once Kotlin gen lands on
// the new Registry.
pub use core::type_binding::TypeBinding;
pub use core::TypeRegistry;
pub use kotlin::{KotlinInterfaceBuilder, KotlinInterfaceGenerator, KotlinTypeMap};
