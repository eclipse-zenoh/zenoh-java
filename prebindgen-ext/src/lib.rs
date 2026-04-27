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
    BodyContext, BodyStrategy, FunctionsBuilder, FunctionsConverter, InlineFn, NameMangler,
    PassThroughBody, StructStrategy, TypeBinding, TypeRegistry, TypesBuilder,
    TypesConverter,
};
pub use jni::{JniDecoderStruct, JniTryClosureBody};
pub use kotlin::{KotlinInterfaceBuilder, KotlinInterfaceGenerator, KotlinTypeMap};
