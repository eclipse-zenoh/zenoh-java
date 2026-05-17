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
use std::sync::Arc;
use std::time::Duration;
use jni::{objects::JClass, JNIEnv};
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

/// Release the Java-held `Arc<Session>` reference. Distinct from
/// `closeSessionViaJNI` (which performs the network shutdown via a
/// borrow); this entry point exists because under the Arc-clone borrow
/// convention the generic generator no longer emits a by-value
/// destructor. Called from `JNISession.close()` after
/// `closeSessionViaJNI` returns.
///
/// # Safety
///
/// `session_ptr` must be the result of an earlier
/// `Arc::into_raw(Arc::new(session))` and must not have been freed.
/// The Kotlin side's `ReentrantReadWriteLock` ensures no borrow is in
/// flight when this runs.
#[no_mangle]
#[allow(non_snake_case)]
pub(crate) unsafe extern "C" fn Java_io_zenoh_jni_JNINative_dropSessionViaJNI(
    _env: JNIEnv,
    _: JClass,
    session_ptr: *const Session,
) {
    Arc::from_raw(session_ptr);
}
