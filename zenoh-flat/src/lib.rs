//! `zenoh-flat` is a placeholder Rust crate for Zenoh flat data support.

pub const PREBINDGEN_OUT_DIR: &str = prebindgen_proc_macro::prebindgen_out_dir!();
pub const FEATURES: &str = prebindgen_proc_macro::features!();

pub mod config;
pub mod errors;
#[cfg(feature = "zenoh-ext")]
pub mod ext;
pub mod jni_converter;
pub mod jni_type_binding;
pub mod session;

