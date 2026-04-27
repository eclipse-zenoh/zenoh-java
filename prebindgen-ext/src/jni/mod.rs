//! JNI-specific strategy implementations and convenience builders for
//! configuring the universal `core::TypesConverter` /
//! `core::FunctionsConverter` to produce JNI wrappers.
//!
//! Concretely:
//! * [`JniDecoderStruct`] — a `StructStrategy` that emits a
//!   `decode_<StructName>` Rust fn per `#[prebindgen]` struct.
//! * [`JniTryClosureBody`] — a `BodyStrategy` that wraps the call in a
//!   try-closure and routes errors through `throw_exception!`.
//! * `inline_fn_helpers` — `pure` / `env_ref` / `env_ref_mut`
//!   constructors for `InlineFn` that hardcode the `&env` / `&mut env`
//!   variable names produced by `JniTryClosureBody`'s prelude.
//! * `opaque` — convenience builders for opaque `&T` borrows and
//!   `Arc<T>` returns, plus `option_of_jobject` for `Option<X>` rows
//!   whose inner is a JNI-object-shaped wire type.

pub mod body_strategy;
pub mod inline_fn_helpers;
pub mod opaque;
pub mod struct_strategy;

pub use body_strategy::JniTryClosureBody;
pub use struct_strategy::JniDecoderStruct;
