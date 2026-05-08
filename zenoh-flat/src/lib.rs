//! `zenoh-flat` is a placeholder Rust crate for Zenoh flat data support.

pub const PREBINDGEN_OUT_DIR: &str = prebindgen_proc_macro::prebindgen_out_dir!();
pub const FEATURES: &str = prebindgen_proc_macro::features!();

pub mod config;
pub mod errors;
pub mod keyexpr;
pub mod sample;
#[cfg(feature = "zenoh-ext")]
pub mod structs;
pub mod session;

// Flat re-exports: every `#[prebindgen]` item is reachable as
// `zenoh_flat::<name>`, so the JNI wrapper generator can call back via a
// single `source_module = "zenoh_flat"` setting without inspecting the
// declaring sub-module.
pub use config::*;
pub use keyexpr::*;
pub use sample::*;
pub use session::*;
#[cfg(feature = "zenoh-ext")]
pub use structs::*;

pub use prebindgen_ext::{core, jni, kotlin};

