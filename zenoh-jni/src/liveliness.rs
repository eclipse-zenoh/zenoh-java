//
// Copyright (c) 2023 ZettaScale Technology
//
// This program and the accompanying materials are made available under the
// terms of the Eclipse Public License 2.0 which is available at
// http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
// which is available at https://www.apache.org/legal/epl-2.0.
//
// SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
//
// Contributors:
//   ZettaScale Zenoh Team, <zenoh@zettascale.tech>
//

//! Liveliness JNI surface.
//!
//! `declare_liveliness_token`, `declare_liveliness_subscriber`, and
//! `liveliness_get` are now generated from `zenoh-flat::liveliness`.
//! Only the destructive `freePtrViaJNI` for `LivelinessToken` remains
//! hand-written here — `LivelinessToken::drop` runs zenoh's undeclare.

use jni::{objects::JClass, JNIEnv};
use zenoh::liveliness::LivelinessToken;

/// Drop the `Box<LivelinessToken>` whose raw pointer `token_ptr` was
/// previously handed to Java. Dropping the token undeclares it on the
/// network.
///
/// # Safety
/// `token_ptr` must be the result of an earlier
/// `Box::into_raw(Box::new(token))` and must not have been freed.
#[no_mangle]
#[allow(non_snake_case)]
pub(crate) unsafe extern "C" fn Java_io_zenoh_jni_JNILivelinessToken_freePtrViaJNI(
    _env: JNIEnv,
    _: JClass,
    token_ptr: *const LivelinessToken,
) {
    drop(Box::from_raw(token_ptr as *mut LivelinessToken));
}
