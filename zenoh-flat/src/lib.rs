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

pub use prebindgen_ext::{core, jni, kotlin};

