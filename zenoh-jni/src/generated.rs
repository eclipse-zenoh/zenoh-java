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

//! Dedicated host for the include of the prebindgen-ext-generated
//! `zenoh_flat_jni.rs`. Carries nothing but the `use` statements the
//! generated code needs in scope and the `include!()` itself — so the
//! generated artifacts (throw fns emitted from `JniExt::prerequisites`,
//! type converters, JNI extern wrappers, opaque-handle helpers) all
//! land in `crate::generated`. External modules in this crate reach
//! them via that path (e.g. `crate::generated::throw_ZError`,
//! `crate::generated::OwnedObject`).

// Types referenced by the generated `zenoh_flat_jni.rs` below must be in scope.
use std::time::Duration;
use zenoh::{
    bytes::Encoding,
    config::Config,
    key_expr::{KeyExpr as ZKeyExpr, SetIntersectionLevel},
    liveliness::LivelinessToken,
    pubsub::{Publisher, Subscriber},
    qos::{CongestionControl, Reliability},
    query::{ConsolidationMode, Querier, Query, QueryTarget, Queryable, Reply, ReplyKeyExpr},
    session::{Session, ZenohId},
};
#[cfg(feature = "zenoh-ext")]
use zenoh_ext::{AdvancedPublisher, AdvancedSubscriber};
// `Priority` resolves to the flat enum here so the auto-generated
// `Priority_to_jint_*` / `jint_to_Priority_*` converters (whose
// signatures use the bare ident) typecheck against `zenoh_flat::Priority`.
// The flat ↔ upstream `zenoh::qos::Priority` value mapping lives in
// `zenoh-flat/src/qos.rs`.
use zenoh_flat::qos::Priority;
use zenoh_flat::sample::Sample;
use zenoh_flat::errors::ZResult;
use zenoh_flat::errors::ZError;
#[cfg(feature = "zenoh-ext")]
use zenoh_flat::structs::{CacheConfig, HistoryConfig, MissDetectionConfig, RecoveryConfig};

include!(concat!(env!("OUT_DIR"), "/zenoh_flat_jni.rs"));
