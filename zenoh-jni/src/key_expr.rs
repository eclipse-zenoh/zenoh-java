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

/// Read the `(ptr, str)` pair from an `io.zenoh.jni.JNIKeyExpr` holder.
unsafe fn read_jni_keyexpr_fields(env: &mut JNIEnv, obj: &JObject) -> ZResult<(i64, String)> {
    let ptr = env
        .get_field(obj, "ptr", "J")
        .and_then(|v| v.j())
        .map_err(|err| zerror!("JNIKeyExpr.ptr: {}", err))?;
    let str_obj = env
        .get_field(obj, "str", "Ljava/lang/String;")
        .and_then(|v| v.l())
        .map_err(|err| zerror!("JNIKeyExpr.str: {}", err))?;
    let s: jni::objects::JString = str_obj.into();
    let binding = env
        .get_string(&s)
        .map_err(|err| zerror!("JNIKeyExpr.str decode: {}", err))?;
    let value = binding
        .to_str()
        .map_err(|err| zerror!("JNIKeyExpr.str utf8: {}", err))?;
    Ok((ptr, value.to_string()))
}

/// Decode an `io.zenoh.jni.JNIKeyExpr` holder into a [`FlatKeyExpr`] for
/// borrow-style consumption: when `ptr != 0`, the JNI side keeps its strong
/// `Arc` reference (the Rust wrapper bumps the count on its own).
pub(crate) unsafe fn decode_jni_keyexpr_borrow(
    env: &mut JNIEnv,
    obj: &JObject,
) -> ZResult<FlatKeyExpr> {
    let (ptr, string) = read_jni_keyexpr_fields(env, obj)?;
    let arc = if ptr != 0 {
        let raw = ptr as *const ZKeyExpr<'static>;
        Arc::increment_strong_count(raw);
        Some(Arc::from_raw(raw))
    } else {
        None
    };
    Ok(FlatKeyExpr { string, ptr: arc })
}

/// Decode an `io.zenoh.jni.JNIKeyExpr` holder into a [`FlatKeyExpr`] for
/// by-value consumption: when `ptr != 0`, the JNI side relinquishes its
/// strong `Arc` reference (the Rust wrapper takes ownership and drops it
/// at end of scope). Used by `undeclare_key_expr` and `drop_key_expr`.
pub(crate) unsafe fn decode_jni_keyexpr_owned(
    env: &mut JNIEnv,
    obj: &JObject,
) -> ZResult<FlatKeyExpr> {
    let (ptr, string) = read_jni_keyexpr_fields(env, obj)?;
    let arc = if ptr != 0 {
        let raw = ptr as *const ZKeyExpr<'static>;
        Some(Arc::from_raw(raw))
    } else {
        None
    };
    Ok(FlatKeyExpr { string, ptr: arc })
}

/// Decode an `io.zenoh.jni.JNIKeyExpr` holder directly into a zenoh
/// [`ZKeyExpr<'static>`]. Used by hand-written JNI fns in modules that
/// haven't been migrated to the auto-generated `FlatKeyExpr` pipeline
/// yet (liveliness, query, querier). Borrow-style: when `ptr != 0`, the
/// JNI side keeps its strong reference; this fn just clones the inner
/// zenoh `KeyExpr`.
pub(crate) unsafe fn decode_jni_key_expr(
    env: &mut JNIEnv,
    obj: &JObject,
) -> ZResult<ZKeyExpr<'static>> {
    let (ptr, string) = read_jni_keyexpr_fields(env, obj)?;
    if ptr != 0 {
        let raw = ptr as *const ZKeyExpr<'static>;
        Arc::increment_strong_count(raw);
        let arc = Arc::from_raw(raw);
        Ok((*arc).clone())
    } else {
        // SAFETY: validated upstream via `try_from` / `autocanonize`.
        Ok(ZKeyExpr::from_string_unchecked(string))
    }
}

/// Encode a [`FlatKeyExpr`] as a freshly-constructed
/// `io.zenoh.jni.JNIKeyExpr` Java object. When `ptr` is `Some`, the strong
/// `Arc` reference is transferred to Java via `Arc::into_raw`.
pub(crate) fn encode_jni_keyexpr(env: &mut JNIEnv, k: FlatKeyExpr) -> ZResult<jobject> {
    let raw_ptr: i64 = match k.ptr {
        Some(arc) => Arc::into_raw(arc) as i64,
        None => 0,
    };
    let jstr = env
        .new_string(k.string)
        .map_err(|err| zerror!(err))?;
    let obj = env
        .new_object(
            "io/zenoh/jni/JNIKeyExpr",
            "(JLjava/lang/String;)V",
            &[JValue::Long(raw_ptr), JValue::Object(&jstr)],
        )
        .map_err(|err| zerror!(err))?;
    Ok(obj.as_raw())
}
