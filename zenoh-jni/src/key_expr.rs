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

/// Decode the JNI key-expression argument into a zenoh
/// [`ZKeyExpr<'static>`]. Used by hand-written JNI fns in modules
/// that haven't been migrated to the auto-generated
/// `impl Into<KeyExpr<'static>>` converter yet (liveliness, query,
/// querier).
///
/// The Kotlin side passes either a boxed `java.lang.Long` (Arc handle)
/// or a `java.lang.String` (raw key-expr text); this fn dispatches on
/// the runtime Java class:
/// * `Long`   — clone the existing `Arc<KeyExpr>` (Java retains its
///              strong reference; per-call decoding is borrow-style).
/// * `String` — validate via `KeyExpr::try_from` and `into_owned()`.
pub(crate) unsafe fn decode_jni_key_expr(
    env: &mut JNIEnv,
    obj: &JObject,
) -> ZResult<ZKeyExpr<'static>> {
    let long_class = env
        .find_class("java/lang/Long")
        .map_err(|err| zerror!("find java.lang.Long: {}", err))?;
    let is_long = env
        .is_instance_of(obj, &long_class)
        .map_err(|err| zerror!("instanceof Long: {}", err))?;
    if is_long {
        let ptr = env
            .call_method(obj, "longValue", "()J", &[])
            .and_then(|v| v.j())
            .map_err(|err| zerror!("Long.longValue: {}", err))?;
        if ptr == 0 {
            return Err(crate::errors::ZError(
                "KeyExpr handle pointer is null".to_string(),
            ));
        }
        let raw = ptr as *const ZKeyExpr<'static>;
        Ok((*raw).clone())
    } else {
        let s: jni::objects::JString = jni::objects::JString::from_raw(obj.as_raw());
        let binding = env
            .get_string(&s)
            .map_err(|err| zerror!("KeyExpr String: {}", err))?;
        let value = binding
            .to_str()
            .map_err(|err| zerror!("KeyExpr utf8: {}", err))?;
        ZKeyExpr::try_from(value)
            .map(|ke| ke.into_owned())
            .map_err(|err| zerror!("KeyExpr parse: {}", err))
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
