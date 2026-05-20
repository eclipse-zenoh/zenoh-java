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

//! Key-expression JNI destructor.
//!
//! Every keyexpr operation is generated from `zenoh-flat::keyexpr` and
//! lives on `JNIKeyExpr` (instance free + companion-object methods). The
//! only piece that stays hand-written is `freePtrViaJNI`: the JniExt
//! borrow-style input convention deliberately does not consume the outer
//! `Box`, so a real drop has to reconstruct the `Box` directly.

use jni::{objects::JClass, JNIEnv};
use zenoh::key_expr::KeyExpr as ZKeyExpr;

/// Drop the `Box<KeyExpr<'static>>` whose raw pointer was previously
/// handed to Java by the auto-generated output converter. Bound from
/// the Kotlin `JNIKeyExpr.free()` helper.
///
/// # Safety
/// `ptr` must be the result of an earlier
/// `Box::into_raw(Box::new(ke))` and must not have been freed.
#[no_mangle]
#[allow(non_snake_case)]
pub(crate) unsafe extern "C" fn Java_io_zenoh_jni_JNIKeyExpr_freePtrViaJNI(
    _env: JNIEnv,
    _: JClass,
    ptr: jni::sys::jlong,
) {
    if ptr != 0 {
        drop(Box::from_raw(ptr as *mut ZKeyExpr<'static>));
    }
}
