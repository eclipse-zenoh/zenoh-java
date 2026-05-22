//! `zenoh-flat` is a placeholder Rust crate for Zenoh flat data support.

pub const PREBINDGEN_OUT_DIR: &str = prebindgen_proc_macro::prebindgen_out_dir!();
pub const FEATURES: &str = prebindgen_proc_macro::features!();

pub mod config;
pub mod errors;
pub mod keyexpr;
pub mod liveliness;
pub mod publisher;
pub mod qos;
pub mod querier;
pub mod query;
pub mod sample;
pub mod session;
#[cfg(feature = "zenoh-ext")]
pub mod structs;

// Flat re-exports: every `#[prebindgen]` item is reachable as
// `zenoh_flat::<name>`, so a downstream binding generator can call back
// via a single `source_module = "zenoh_flat"` setting without inspecting
// the declaring sub-module.
pub use config::*;
pub use keyexpr::*;
pub use liveliness::*;
pub use publisher::*;
pub use qos::*;
pub use querier::*;
pub use query::*;
pub use sample::*;
pub use session::*;
#[cfg(feature = "zenoh-ext")]
pub use structs::*;

