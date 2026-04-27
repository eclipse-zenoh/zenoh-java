//! Kotlin interface declaration generator.
//!
//! Reads the same item stream that drives the Rust converters, plus a
//! [`KotlinTypeMap`] (Rust type-shape → Kotlin name) and the universal
//! [`TypeRegistry`] (used for wire-side metadata like `is_option` /
//! `is_pointer`), and emits a `.kt` file containing the JNI surface as
//! `data class` definitions and `external fun` prototypes inside an
//! `internal object`.

pub mod interface_generator;
pub mod type_map;

pub use interface_generator::{KotlinInterfaceBuilder, KotlinInterfaceGenerator};
pub use type_map::KotlinTypeMap;
