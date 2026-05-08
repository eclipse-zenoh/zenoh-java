//! Universal converters for `#[prebindgen]` items, plus destination-
//! language helpers (JNI strategies and Kotlin interface generation).
//!
//! See [`core`] for the language-agnostic `TypesConverter` /
//! `FunctionsConverter` primitives, [`jni`] for the JNI-flavoured
//! strategies and convenience builders, and [`kotlin`] for the
//! Kotlin interface declaration generator.

pub mod core;
pub mod jni;
pub mod kotlin;
mod util;

pub use core::{
    BodyContext, BodyStrategy, FunctionsBuilder, FunctionsConverter, NameMangler,
    PassThroughBody, StructStrategy, TypeRegistry, TypesBuilder, TypesConverter,
};
pub use core::type_binding::TypeBinding;
pub use core::type_registry::{
    input_option, output_option, option_wire_type, nullable_to_option, option_to_nullable,
    input_result, output_result, result_wire_type,
};
pub use jni::{JniDecoderStruct, JniTryClosureBody};
pub use kotlin::{KotlinInterfaceBuilder, KotlinInterfaceGenerator, KotlinTypeMap};
