//! Core: language-agnostic primitives for the Registry-based pipeline.
//!
//! Pipeline shape:
//!   1. [`registry::Registry::from_source`] — scan a `prebindgen::Source`
//!      into a flat type table.
//!   2. [`resolve::resolve`] — fixed-point loop driving the configured
//!      [`prebindgen_ext::PrebindgenExt`] across all rank phases and both
//!      directions.
//!   3. [`write::write_rust`] — emit the resolved bindings file.
//!   4. (Destination-language Kotlin emission lives in `crate::kotlin`.)
//!
//! `inline_fn`, `name_mangler`, `type_binding`, `type_registry` are kept
//! transitionally; they're only referenced by the unmigrated
//! [`crate::kotlin::KotlinInterfaceGenerator`] which still consumes the
//! old `TypeRegistry` shape. They will be removed once that generator
//! lands on the new `Registry`.

pub mod inline_fn;
pub mod name_mangler;
pub mod niches;
pub mod prebindgen_ext;
pub mod registry;
pub mod resolve;
pub mod type_binding;
pub mod type_registry;
pub mod write;

pub use niches::{NicheSlot, Niches};

pub use type_registry::{
    input_option, input_result, nullable_to_option, option_to_nullable, option_wire_type,
    output_option, output_result, primitive_builtins, result_wire_type, TypeRegistry,
};
