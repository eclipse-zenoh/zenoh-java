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

// Types referenced by the generated `zenoh_flat_jni.rs` below must be in scope.
use std::time::Duration;
use zenoh::{
    bytes::Encoding,
    config::Config,
    key_expr::{KeyExpr as ZKeyExpr, SetIntersectionLevel},
    pubsub::{Publisher, Subscriber},
    qos::{CongestionControl, Priority, Reliability},
    query::{ConsolidationMode, Querier, Query, Queryable, QueryTarget, Reply, ReplyKeyExpr},
    session::{Session, ZenohId},
};
use zenoh_flat::sample::Sample;
#[cfg(feature = "zenoh-ext")]
use zenoh_ext::{AdvancedPublisher, AdvancedSubscriber};
#[cfg(feature = "zenoh-ext")]
use zenoh_flat::structs::{CacheConfig, HistoryConfig, MissDetectionConfig, RecoveryConfig};

include!(concat!(env!("OUT_DIR"), "/zenoh_flat_jni.rs"));
