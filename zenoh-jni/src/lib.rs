//
// Copyright (c) 2023 ZettaScale Technology
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

#[macro_use]
extern crate zenoh_flat;

mod config;
#[cfg(feature = "zenoh-ext")]
pub(crate) mod ext;
// `generated` is the dedicated home for `include!`-ing the
// prebindgen-ext-generated `zenoh_flat_jni.rs`. Holds the `use` aliases
// the generated code needs in scope and emits every generated symbol
// under `crate::generated::*` — including the per-exception throw fns
// (`throw_ZError`, …) that hand-written modules in this crate call to
// surface a JVM exception. Replaces the hand-written `errors.rs`
// (`ThrowOnJvm` trait + impl + `throw_exception!` macro) end-to-end.
mod generated;
mod logger;
pub(crate) mod sample_callback;
mod scouting;
mod utils;
#[cfg(feature = "zenoh-ext")]
mod zbytes;
#[cfg(feature = "zenoh-ext")]
mod zbytes_kotlin;
mod zenoh_id;

// Test should be runned with `cargo test --no-default-features`
#[test]
#[cfg(not(feature = "default"))]
fn test_no_default_features() {
    assert_eq!(zenoh::FEATURES, concat!(" zenoh/unstable"));
}
