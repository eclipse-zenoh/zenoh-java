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

//! Query JNI surface.
//!
//! `reply_success`, `reply_error`, and `reply_delete` are now generated
//! from `zenoh-flat::query`. Only the destructive `freePtrViaJNI`
//! remains hand-written here: the JniExt consume-style input convention
//! invalidates the Java-side handle atomically, so `freePtrViaJNI` is
//! only reached when the user closes a query without ever replying.

use jni::{objects::JClass, JNIEnv};
use zenoh::query::Query;

/// Drop the `Box<Query>` whose raw pointer `query_ptr` was previously
/// handed to Java. Dropping the `Query` releases the underlying zenoh
/// resources without sending a reply.
///
/// # Safety
/// `query_ptr` must be the result of an earlier
/// `Box::into_raw(Box::new(query))` and must not have been freed.
#[no_mangle]
#[allow(non_snake_case)]
pub(crate) unsafe extern "C" fn Java_io_zenoh_jni_JNIQuery_freePtrViaJNI(
    _env: JNIEnv,
    _: JClass,
    query_ptr: *const Query,
) {
    drop(Box::from_raw(query_ptr as *mut Query));
}
