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

pub mod errors;
pub mod key_expr;
pub mod liveliness;
pub mod logger;
pub mod publisher;
pub mod querier;
pub mod query;
pub mod queryable;
pub mod session;
pub mod subscriber;
pub mod utils;
pub mod zenoh_id;

pub use errors::{ZError, ZResult};

// Re-export upstream zenoh so dependents (outdated/ shim in zenoh-flat-jni)
// don't need a separate `zenoh` Cargo dep to name handle types.
pub use zenoh;

// Test should be runned with `cargo test --no-default-features`
#[test]
#[cfg(not(feature = "default"))]
fn test_no_default_features() {
    assert_eq!(zenoh::FEATURES, concat!(" zenoh/unstable"));
}
