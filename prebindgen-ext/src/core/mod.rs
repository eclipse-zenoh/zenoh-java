//! Universal, language-agnostic primitives for converting `#[prebindgen]`
//! items into a destination Rust include file.
//!
//! Two converters split the work along the natural axis:
//!
//! * [`TypesConverter`] consumes `#[prebindgen]` `syn::ItemStruct`s and
//!   delegates per-struct emission to a [`StructStrategy`].
//! * [`FunctionsConverter`] consumes `#[prebindgen]` `syn::ItemFn`s and
//!   delegates per-function body emission to a [`BodyStrategy`].
//!
//! Both converters share a [`TypeRegistry`] of [`TypeBinding`]s that
//! describes how each Rust type-shape is carried across the FFI boundary.
//!
//! Two reference targets are expected to be expressible as configurations
//! of these converters:
//!
//! 1. JNI/Kotlin (today), via the `prebindgen-ext::jni` and
//!    `prebindgen-ext::kotlin` modules.
//! 2. C/cbindgen (future), via a `ReprCStruct` strategy + `PassThroughBody`
//!    body strategy + `NameMangler::Identity`. See the doc comment on
//!    [`FunctionsConverter`] for the mapping sketch.

pub mod converter_name;
pub mod functions_converter;
pub mod inline_fn;
pub mod name_mangler;
pub mod prebindgen_ext;
pub mod registry;
pub mod resolve;
pub mod type_binding;
pub mod type_registry;
pub mod types_converter;
pub mod write;

pub use functions_converter::{
    BodyContext, BodyStrategy, FunctionsBuilder, FunctionsConverter, PassThroughBody,
};
pub use inline_fn::{InputFn, OutputFn, NO_INPUT, NO_OUTPUT};
pub use name_mangler::NameMangler;
pub use type_registry::{
    primitive_builtins, TypeRegistry,
    input_result, output_result, result_wire_type,
};
pub use types_converter::{StructStrategy, TypesBuilder, TypesConverter};
