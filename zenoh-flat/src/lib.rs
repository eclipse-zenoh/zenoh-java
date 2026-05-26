//
// Copyright (c) 2026 ZettaScale Technology
//
// This program and the accompanying materials are made available under the
// terms of the Eclipse Public License 2.0 which is available at
// http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
// which is available at https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
//
// Contributors:
//   ZettaScale Zenoh Team, <zenoh@zettascale.tech>
//

pub const PREBINDGEN_OUT_DIR: &str = prebindgen_proc_macro::prebindgen_out_dir!();
pub const FEATURES: &str = prebindgen_proc_macro::features!();

pub(crate) mod keyexpr;
pub(crate) mod z_keyexpr;
pub(crate) mod z_config;
pub(crate) mod whatami;
pub(crate) mod z_zenoh_id;
pub(crate) mod zenoh_id;
pub(crate) mod z_hello;
pub(crate) mod hello;
pub(crate) mod z_scout;
pub(crate) mod error;
pub(crate) mod logger;

// reexports to make all zenoh-flat API really flat
pub use keyexpr::*;
pub use z_keyexpr::*;
pub use z_config::*;
pub use whatami::*;
pub use z_zenoh_id::*;
pub use zenoh_id::*;
pub use z_hello::*;
pub use hello::*;
pub use z_scout::*;
pub use error::*;
pub use logger::*;

// reexports of zenoh types with Z prefix to distiguish them from zenoh-flat types
pub type ZKeyExpr = zenoh::key_expr::KeyExpr<'static>;
pub type ZConfig = zenoh::Config;
pub type ZZenohId = zenoh::session::ZenohId;
pub type ZHello = zenoh::scouting::Hello;
pub type ZScout = zenoh::scouting::Scout<()>;