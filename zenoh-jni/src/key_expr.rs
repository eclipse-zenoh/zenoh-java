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

use jni::{objects::JString, JNIEnv};
use zenoh::key_expr::KeyExpr;

use crate::errors::ZResult;
use crate::utils::decode_string;
use crate::zerror;

/// Bridge a Kotlin-side `(KeyExpr.flat.keyExprNative, KeyExpr.flat.keyExprString)`
/// pair into a `zenoh::KeyExpr<'static>`. When the pointer is non-null,
/// the layout matches `Box<KeyExpr<'static>>` (compatible across
/// zenoh-jni and zenoh-flat-jni), so cloning the pointee yields a fresh
/// owned key expression; otherwise the string is parsed fresh.
///
/// # Safety
///
/// `key_expr_ptr` must either be null or point to a live
/// `KeyExpr<'static>` produced by either side of the JNI boundary. The
/// string is assumed already validated by the Kotlin wrapper.
pub(crate) unsafe fn process_kotlin_key_expr(
    env: &mut JNIEnv,
    key_expr_str: &JString,
    key_expr_ptr: *const KeyExpr<'static>,
) -> ZResult<KeyExpr<'static>> {
    if key_expr_ptr.is_null() {
        let key_expr = decode_string(env, key_expr_str)
            .map_err(|err| zerror!("Unable to get key expression string value: '{}'.", err))?;
        Ok(KeyExpr::from_string_unchecked(key_expr))
    } else {
        Ok((*key_expr_ptr).clone())
    }
}
