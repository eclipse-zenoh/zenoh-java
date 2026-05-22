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

use zenoh_flat::errors::ZResult;
use jni::{
    objects::{JByteArray, JObject, JString},
    sys::jint,
    JNIEnv, JavaVM,
};
use zenoh::{
    bytes::{Encoding, ZBytes},
    internal::buffers::ZSlice,
};

pub(crate) fn decode_encoding(
    env: &mut JNIEnv,
    encoding: jint,
    schema: &JString,
) -> ZResult<Encoding> {
    let schema: Option<ZSlice> = if schema.is_null() {
        None
    } else {
        // Inline String decoding logic (moved from decode_string)
        let binding = env
            .get_string(schema)
            .map_err(|err| zerror!("Error while retrieving JString: {}", err))?;
        let value = binding
            .to_str()
            .map_err(|err| zerror!("Error decoding JString: {}", err))?;
        Some(value.to_string().into_bytes().into())
    };
    let encoding_id =
        u16::try_from(encoding).map_err(|err| zerror!("Failed to decode encoding: {}", err))?;
    Ok(Encoding::new(encoding_id, schema))
}

/// Decode a Kotlin `io.zenoh.jni.JNIEncoding` holder into a zenoh [`Encoding`].
/// Fields: `id: Int`, `schema: String?`.
pub(crate) fn decode_jni_encoding(env: &mut JNIEnv, obj: &JObject) -> ZResult<Encoding> {
    let id = env
        .get_field(obj, "id", "I")
        .and_then(|v| v.i())
        .map_err(|err| zerror!("JNIEncoding.id: {}", err))?;
    let schema_obj = env
        .get_field(obj, "schema", "Ljava/lang/String;")
        .and_then(|v| v.l())
        .map_err(|err| zerror!("JNIEncoding.schema: {}", err))?;
    let schema_js: JString = schema_obj.into();
    decode_encoding(env, id, &schema_js)
}

pub(crate) fn get_java_vm(env: &mut JNIEnv) -> ZResult<JavaVM> {
    env.get_java_vm()
        .map_err(|err| zerror!("Unable to retrieve JVM reference: {}", err))
}

pub(crate) fn get_callback_global_ref(
    env: &mut JNIEnv,
    callback: &JObject,
) -> zenoh_flat::errors::ZResult<jni::objects::GlobalRef> {
    env.new_global_ref(callback)
        .map_err(|err| zerror!("Unable to get reference to the provided callback: {}", err))
}

/// Helper function to convert a JByteArray into a Vec<u8>.
pub(crate) fn decode_byte_array(env: &JNIEnv, payload: &JByteArray) -> ZResult<Vec<u8>> {
    let payload_len = env
        .get_array_length(payload)
        .map(|length| length as usize)
        .map_err(|err| zerror!(err))?;
    let mut buff = vec![0; payload_len];
    env.get_byte_array_region(payload, 0, &mut buff[..])
        .map_err(|err| zerror!(err))?;
    let buff: Vec<u8> = unsafe { std::mem::transmute::<Vec<i8>, Vec<u8>>(buff) };
    Ok(buff)
}

pub(crate) fn bytes_to_java_array<'a>(env: &JNIEnv<'a>, slice: &ZBytes) -> ZResult<JByteArray<'a>> {
    env.byte_array_from_slice(&slice.to_bytes())
        .map_err(|err| zerror!(err))
}

pub(crate) fn slice_to_java_string<'a>(env: &JNIEnv<'a>, slice: &ZSlice) -> ZResult<JString<'a>> {
    env.new_string(
        String::from_utf8(slice.to_vec())
            .map_err(|err| zerror!("Unable to decode string: {}", err))?,
    )
    .map_err(|err| zerror!(err))
}

/// A type that calls a function when dropped
pub(crate) struct CallOnDrop<F: FnOnce()>(core::mem::MaybeUninit<F>);
impl<F: FnOnce()> CallOnDrop<F> {
    /// Constructs a value that calls `f` when dropped.
    pub fn new(f: F) -> Self {
        Self(core::mem::MaybeUninit::new(f))
    }
    /// Does nothing, but tricks closures into moving the value inside,
    /// so that the closure's destructor will call `drop(self)`.
    pub fn noop(&self) {}
}
impl<F: FnOnce()> Drop for CallOnDrop<F> {
    fn drop(&mut self) {
        // Take ownership of the closure that is always initialized,
        // since the only constructor uses `MaybeUninit::new`
        let f = unsafe { self.0.assume_init_read() };
        // Call the now owned function
        f();
    }
}

/// Wrap a decoded data callback so that the supplied Kotlin `on_close`
/// callback fires exactly once when the data closure is dropped (via
/// `CallOnDrop`). Used by hand-written JNI entry points whose Kotlin signatures
/// pair a callback with an `on_close: JNICallback`. Generated entry
/// points (via `JniMethodsConverter`) take `on_close` as a separate Rust parameter
/// instead and are wrapped at the zenoh-flat layer.
pub(crate) fn wrap_with_on_close<T, F>(
    env: &mut JNIEnv,
    on_close: JObject,
    cb: F,
) -> ZResult<impl Fn(T) + Send + Sync + 'static>
where
    F: Fn(T) + Send + Sync + 'static,
{
    let java_vm = Arc::new(get_java_vm(env)?);
    let on_close_global_ref = get_callback_global_ref(env, &on_close)?;
    let guard = load_on_close(&java_vm, on_close_global_ref);
    Ok(move |t| {
        guard.noop();
        cb(t);
    })
}

pub(crate) fn load_on_close(
    java_vm: &Arc<jni::JavaVM>,
    on_close_global_ref: jni::objects::GlobalRef,
) -> CallOnDrop<impl FnOnce()> {
    CallOnDrop::new({
        let java_vm = java_vm.clone();
        move || {
            let mut env = match java_vm.attach_current_thread_as_daemon() {
                Ok(env) => env,
                Err(err) => {
                    tracing::error!("Unable to attach thread for 'onClose' callback: {}", err);
                    return;
                }
            };
            match env.call_method(on_close_global_ref, "run", "()V", &[]) {
                Ok(_) => (),
                Err(err) => {
                    _ = env.exception_describe();
                    crate::generated::throw_ZError(
                        &mut env,
                        &zerror!("Error while running 'onClose' callback: {}", err),
                    );
                }
            }
        }
    })
}
