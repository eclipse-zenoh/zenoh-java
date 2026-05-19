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

//! Querier JNI surface.
//!
//! `querier_get` is now generated from `zenoh-flat::querier`. Only the
//! destructive `freePtrViaJNI` remains hand-written here.

use jni::{objects::JClass, JNIEnv};
use zenoh::query::Querier;

/// Drop the `Box<Querier>` whose raw pointer `querier_ptr` was
/// previously handed to Java. Dropping the `Querier` triggers zenoh's
/// network undeclare.
///
/// # Safety
/// `querier_ptr` must be the result of an earlier
/// `Box::into_raw(Box::new(querier))` and must not have been freed.
#[no_mangle]
#[allow(non_snake_case)]
pub(crate) unsafe extern "C" fn Java_io_zenoh_jni_JNIQuerier_freePtrViaJNI(
    _env: JNIEnv,
    _: JClass,
    querier_ptr: *const Querier<'static>,
) {
    drop(Box::from_raw(querier_ptr as *mut Querier<'static>));
}
