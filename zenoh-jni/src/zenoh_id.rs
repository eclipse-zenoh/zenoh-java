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

use crate::{errors::ZResult, throw_exception, utils::decode_byte_array};
use jni::{
    objects::{JByteArray, JClass, JList, JString},
    sys::{jbyteArray, jobject, jstring},
    JNIEnv,
};
use zenoh::session::ZenohId;

/// Encode a single [`ZenohId`] as a Java `byte[]`.
pub(crate) fn zenoh_id_to_byte_array(env: &JNIEnv<'_>, zid: ZenohId) -> ZResult<jbyteArray> {
    env.byte_array_from_slice(&zid.to_le_bytes())
        .map(|x| x.as_raw())
        .map_err(|err| zerror!(err))
}

/// Encode a `Vec<ZenohId>` as a Java `ArrayList<byte[]>`.
pub(crate) fn zenoh_ids_to_java_list(env: &mut JNIEnv, ids: Vec<ZenohId>) -> ZResult<jobject> {
    let array_list = env
        .new_object("java/util/ArrayList", "()V", &[])
        .map_err(|err| zerror!(err))?;
    let jlist = JList::from_env(env, &array_list).map_err(|err| zerror!(err))?;
    for id in ids {
        let value = &mut env
            .byte_array_from_slice(&id.to_le_bytes())
            .map_err(|err| zerror!(err))?;
        jlist.add(env, value).map_err(|err| zerror!(err))?;
    }
    Ok(array_list.as_raw())
}

/// Returns the string representation of a ZenohID.
#[no_mangle]
#[allow(non_snake_case)]
pub extern "C" fn Java_io_zenoh_jni_JNIZenohId_toStringViaJNI(
    mut env: JNIEnv,
    _class: JClass,
    zenoh_id: JByteArray,
) -> jstring {
    || -> ZResult<JString> {
        let bytes = decode_byte_array(&env, zenoh_id)?;
        let zenohid = ZenohId::try_from(bytes.as_slice()).map_err(|err| zerror!(err))?;
        env.new_string(zenohid.to_string())
            .map_err(|err| zerror!(err))
    }()
    .unwrap_or_else(|err| {
        throw_exception!(env, err);
        JString::default()
    })
    .as_raw()
}
