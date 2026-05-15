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
//! Publisher JNI surface.
//!
//! `put` and `delete` are now generated from `zenoh-flat::publisher`. Only
//! the destructive `freePtrViaJNI` remains hand-written here: the JniExt
//! borrow-style input convention deliberately does not consume the outer
//! `Arc`, so a real drop must reconstruct the `Arc` directly.

use std::sync::Arc;

use jni::{objects::JClass, JNIEnv};
use zenoh::pubsub::Publisher;

/// Decrement the strong count of the `Arc<Publisher>` whose raw pointer
/// `publisher_ptr` was previously handed to Java. When the count reaches
/// zero the `Publisher` is dropped, which triggers zenoh's network
/// undeclare.
///
/// # Safety
/// `publisher_ptr` must be the result of an earlier
/// `Arc::into_raw(Arc::new(publisher))` and must not have been freed.
#[no_mangle]
#[allow(non_snake_case)]
pub(crate) unsafe extern "C" fn Java_io_zenoh_jni_JNIPublisher_freePtrViaJNI(
    _env: JNIEnv,
    _: JClass,
    publisher_ptr: *const Publisher,
) {
    Arc::from_raw(publisher_ptr);
}
