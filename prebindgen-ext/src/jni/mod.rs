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
//!   constructors for param-direction `TypeBinding` rows that hardcode
//!   the `&env` / `&mut env` variable names produced by
//!   `JniTryClosureBody`'s prelude.
//! * `jni_type` — helpers for common JNI wire-type spellings like
//!   `JObject`, `JString`, and `jint`.
//! * `jni_type_helper` — type-specific binding constructors like
//!   `jobject(rust_type, decode)` and `jint(rust_type, decode)`.
//! * `opaque` — convenience builders for opaque `&T` borrows and
//!   `Arc<T>` returns, plus `option_of_jobject` for `Option<X>` rows
//!   whose inner is a JNI-object-shaped wire type.

pub mod body_strategy;
pub mod byte_array_helpers;
pub mod callback_strategy;
pub mod inline_fn_helpers;
pub mod jni_ext;
pub mod jni_type;
pub mod jni_type_helper;
pub mod opaque;
pub mod string_helpers;
pub mod struct_strategy;
pub(crate) mod wire_access;

pub use body_strategy::JniTryClosureBody;
pub use byte_array_helpers::{decode_byte_array, encode_byte_array, null_byte_array};
pub use callback_strategy::{CallbackHelpers, CallbacksBuilder, CallbacksConverter};
pub use jni_ext::JniExt;
pub use string_helpers::{decode_string, encode_string, null_string};
pub use struct_strategy::JniDecoderStruct;
