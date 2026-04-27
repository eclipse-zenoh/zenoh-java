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

use jni::objects::{JObject, JString};
use jni::JNIEnv;
use zenoh::key_expr::KeyExpr;

use crate::errors::ZResult;
use crate::owned_object::OwnedObject;
use crate::utils::decode_string;

/// Materialize a [`KeyExpr<'static>`] from its dual JNI representation:
/// when `key_expr_ptr` is non-null it points to a session-declared
/// `Arc<KeyExpr>`; otherwise the expression is rebuilt from the already
/// validated string form.
///
/// # Safety
/// The `key_expr_str` argument should already have been validated upon
/// creation of the `KeyExpr` instance on Kotlin.
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
        let key_expr = OwnedObject::from_raw(key_expr_ptr);
        Ok((*key_expr).clone())
    }
}

/// Decode a Kotlin `io.zenoh.jni.JNIKeyExpr` holder into a
/// [`KeyExpr<'static>`]. The holder carries `ptr: Long` and `str: String`;
/// `ptr != 0` means the KeyExpr was declared on a session and the pointer is
/// an `Arc::into_raw(Arc::new(KeyExpr))`; `ptr == 0` means to build the
/// expression from the string. Registered as the input decoder for
/// `KeyExpr<'static>` ↔ `JObject` in `build.rs`.
pub(crate) unsafe fn decode_jni_key_expr(
    env: &mut JNIEnv,
    obj: &JObject,
) -> ZResult<KeyExpr<'static>> {
    let ptr = env
        .get_field(obj, "ptr", "J")
        .and_then(|v| v.j())
        .map_err(|err| zerror!("JNIKeyExpr.ptr: {}", err))?;
    let str_obj = env
        .get_field(obj, "str", "Ljava/lang/String;")
        .and_then(|v| v.l())
        .map_err(|err| zerror!("JNIKeyExpr.str: {}", err))?;
    let str_js: JString = str_obj.into();
    process_kotlin_key_expr(env, &str_js, ptr as *const KeyExpr<'static>)
}
