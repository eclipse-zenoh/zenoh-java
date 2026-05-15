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

use jni::objects::{JClass, JObject};
use jni::JNIEnv;
use zenoh::key_expr::KeyExpr as ZKeyExpr;

use crate::errors::ZResult;

/// Decode an `io.zenoh.jni.KeyExpr` holder directly into a zenoh
/// [`ZKeyExpr<'static>`]. Used by hand-written JNI fns in modules that
/// haven't been migrated to the auto-generated `impl Into<KeyExpr>`
/// converter yet (liveliness, query, querier). Borrow-style: when
/// `ptr != 0`, the JNI side keeps its strong reference; this fn just
/// clones the inner zenoh `KeyExpr`.
pub(crate) unsafe fn decode_jni_key_expr(
    env: &mut JNIEnv,
    obj: &JObject,
) -> ZResult<ZKeyExpr<'static>> {
    let ptr = env
        .get_field(obj, "ptr", "J")
        .and_then(|v| v.j())
        .map_err(|err| zerror!("KeyExpr.ptr: {}", err))?;
    if ptr != 0 {
        let raw = ptr as *const ZKeyExpr<'static>;
        Ok((*raw).clone())
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

/// Decrement the strong count of the `Arc<KeyExpr<'static>>` whose raw
/// pointer was previously handed to Java by the auto-generated output
/// converter. Bound from the Kotlin `KeyExpr.close()` helper.
///
/// Needed because the auto-generated input converter for
/// `impl Into<KeyExpr<'static>>` deliberately does not consume the
/// outer `Arc` (it only clones the value through the pointer).
///
/// # Safety
/// `ptr` must be the result of an earlier
/// `Arc::into_raw(Arc::new(ke))` and must not have been freed.
#[no_mangle]
#[allow(non_snake_case)]
pub(crate) unsafe extern "C" fn Java_io_zenoh_jni_JNINative_dropKeyExprViaJNI(
    _env: JNIEnv,
    _: JClass,
    ptr: jni::sys::jlong,
) {
    if ptr != 0 {
        let _ = Arc::from_raw(ptr as *const ZKeyExpr<'static>);
    }
}
