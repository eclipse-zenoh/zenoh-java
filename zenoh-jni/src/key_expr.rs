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

use std::sync::Arc;

use jni::objects::{JObject, JValue};
use jni::sys::jobject;
use jni::JNIEnv;
use zenoh::key_expr::KeyExpr as ZKeyExpr;
use zenoh_flat::keyexpr::KeyExpr as FlatKeyExpr;

use crate::errors::ZResult;

unsafe fn clone_key_expr_from_arc_ptr(ptr: i64) -> ZResult<ZKeyExpr<'static>> {
    let raw = ptr as *const ZKeyExpr<'static>;
    Ok((*raw).clone())
}

/// Encode a [`FlatKeyExpr`] as a freshly-constructed
/// `io.zenoh.jni.KeyExpr` Java object. When `ptr` is non-zero, the strong
/// `Arc` reference (already owned by this value) is transferred to Java.
pub(crate) fn encode_jni_keyexpr(env: &mut JNIEnv, k: FlatKeyExpr) -> ZResult<jobject> {
    let raw_ptr = k
        .ptr
        .map(|key_expr| Arc::into_raw(Arc::new(key_expr)) as i64)
        .unwrap_or(0);
    let jstr = env
        .new_string(k.string)
        .map_err(|err| zerror!(err))?;
    let obj = env
        .new_object(
            "io/zenoh/jni/KeyExpr",
            "(JLjava/lang/String;)V",
            &[JValue::Long(raw_ptr), JValue::Object(&jstr)],
        )
        .map_err(|err| zerror!(err))?;
    Ok(obj.as_raw())
}

/// Decode an `io.zenoh.jni.KeyExpr` holder directly into a zenoh
/// [`ZKeyExpr<'static>`]. Used by hand-written JNI fns in modules that
/// haven't been migrated to the auto-generated `FlatKeyExpr` pipeline
/// yet (liveliness, query, querier). Borrow-style: when `ptr != 0`, the
/// JNI side keeps its strong reference; this fn just clones the inner
/// zenoh `KeyExpr`.
pub(crate) unsafe fn decode_jni_key_expr(
    env: &mut JNIEnv,
    obj: &JObject,
) -> ZResult<ZKeyExpr<'static>> {
    let ptr = env
        .get_field(obj, "ptr", "J")
        .and_then(|v| v.j())
        .map_err(|err| zerror!("KeyExpr.ptr: {}", err))?;
    if ptr != 0 {
        clone_key_expr_from_arc_ptr(ptr)
    } else {
        let str_obj = env
            .get_field(obj, "string", "Ljava/lang/String;")
            .and_then(|v| v.l())
            .map_err(|err| zerror!("KeyExpr.string: {}", err))?;
        let s: jni::objects::JString = str_obj.into();
        let binding = env
            .get_string(&s)
            .map_err(|err| zerror!("KeyExpr.string decode: {}", err))?;
        let value = binding
            .to_str()
            .map_err(|err| zerror!("KeyExpr.string utf8: {}", err))?;
        // SAFETY: validated upstream via `try_from` / `autocanonize`.
        Ok(ZKeyExpr::from_string_unchecked(value.to_string()))
    }
}
