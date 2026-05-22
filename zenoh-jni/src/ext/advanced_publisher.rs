//
// Copyright (c) 2026 ZettaScale Technology
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

use jni::objects::JValue;
use jni::{
    objects::{JByteArray, JClass},
    JNIEnv,
};
use zenoh::handlers::{Callback as ZCallback, DefaultHandler as ZDefaultHandler};
use zenoh::Wait as ZWait;
use zenoh_ext::AdvancedPublisher as ZAdvancedPublisher;

use crate::generated::OwnedObject;
use crate::utils::{get_callback_global_ref, get_java_vm, load_on_close};


use zenoh_flat::errors::ZResult;

use crate::utils::{decode_byte_array, decode_jni_encoding};
use jni::sys::jboolean;
use std::ptr::null;

use jni::objects::JObject;
use zenoh::matching::{
    MatchingListener as ZMatchingListener, MatchingListenerBuilder as ZMatchingListenerBuilder,
    MatchingStatus as ZMatchingStatus,
};

trait SetJniMatchingStatusCallback {
    type WithCallback;

    unsafe fn set_jni_matching_status_callback(
        self,
        env: &mut JNIEnv,
        callback: JObject,
        on_close: JObject,
    ) -> ZResult<Self::WithCallback>;
}

impl<'a> SetJniMatchingStatusCallback for ZMatchingListenerBuilder<'a, ZDefaultHandler> {
    type WithCallback = ZMatchingListenerBuilder<'a, ZCallback<ZMatchingStatus>>;

    unsafe fn set_jni_matching_status_callback(
        self,
        env: &mut JNIEnv,
        callback: JObject,
        on_close: JObject,
    ) -> ZResult<Self::WithCallback> {
        let java_vm = Arc::new(get_java_vm(env)?);
        let callback_global_ref = get_callback_global_ref(env, &callback)?;
        let on_close_global_ref = get_callback_global_ref(env, &on_close)?;
        let on_close = load_on_close(&java_vm, on_close_global_ref);

        let builder = self.callback(move |matching_status| {
            on_close.noop(); // Moves `on_close` inside the closure so it gets destroyed with the closure
            let _ = || -> ZResult<()> {
                let mut env = java_vm.attach_current_thread_as_daemon().map_err(|err| {
                    zerror!("Unable to attach thread for matching listener: {}", err)
                })?;

                env.call_method(
                    &callback_global_ref,
                    "run",
                    "(Z)V",
                    &[JValue::from(matching_status.matching())],
                )
                .map_err(|err| zerror!(err))?;
                Ok(())
            }()
            .map_err(|err| tracing::error!("On matching listener callback error: {err}"));
        });
        Ok(builder)
    }
}

/// Declare a ZMatchingListener for [ZAdvancedPublisher] via JNI.
///
/// Parameters:
/// - `env`: The JNI environment.
/// - `_class`: The JNI class.
/// - `advanced_publisher_ptr`: The raw pointer to an [ZAdvancedPublisher].
/// - `callback`: The callback function as an instance of the `JNIMatchingStatusCallback` interface in Java/Kotlin.
/// - `on_close`: A Java/Kotlin `JNICallback` function interface to be called upon undeclaring the [ZMatchingListener].
///
/// Returns:
/// - A raw pointer to the declared [ZMatchingListener]. In case of failure, an exception is thrown and null is returned.
///
/// Safety:
/// - The function is marked as unsafe due to raw pointer manipulation and JNI interaction.
/// - It assumes that the provided [ZAdvancedPublisher] pointer is valid and has not been modified or freed.
/// - The [ZAdvancedPublisher] pointer remains valid and the ownership of the [ZAdvancedPublisher] is not transferred,
///   allowing safe usage of the [ZAdvancedPublisher] after this function call.
/// - The callback function passed as `callback` must be a valid instance of the `JNIMatchingStatusCallback` interface
///   in Java/Kotlin, matching the specified signature.
/// - The function may throw a JNI exception in case of failure, which should be handled by the caller.
///
#[cfg(feature = "zenoh-ext")]
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_io_zenoh_jni_JniZAdvancedPublisher_declareMatchingListenerViaJNI(
    mut env: JNIEnv,
    _class: JClass,
    advanced_publisher_ptr: *const ZAdvancedPublisher,

    callback: JObject,
    on_close: JObject,
) -> *const ZMatchingListener<()> {
    let advanced_publisher = OwnedObject::from_raw(advanced_publisher_ptr);

    || -> ZResult<*const ZMatchingListener<()>> {
        tracing::debug!(
            "Declaring matching listener on '{}'...",
            advanced_publisher.key_expr()
        );

        let matching_listener = advanced_publisher
            .matching_listener()
            .set_jni_matching_status_callback(&mut env, callback, on_close)?
            .wait()
            .map_err(|err| zerror!("Unable to declare matching listener: {}", err))?;

        tracing::debug!(
            "Matching listener declared on '{}'...",
            advanced_publisher.key_expr()
        );
        Ok(Box::into_raw(Box::new(matching_listener)) as *const ZMatchingListener<()>)
    }()
    .unwrap_or_else(|err| {
        crate::generated::throw_ZError(&mut env, &err);
        null()
    })
}

/// Declare a background matching listener for [ZAdvancedPublisher] via JNI.
/// Register the listener callback to be run in background until the [ZAdvancedPublisher] is undeclared.
///
/// Parameters:
/// - `env`: The JNI environment.
/// - `_class`: The JNI class.
/// - `advanced_publisher_ptr`: The raw pointer to an [ZAdvancedPublisher].
/// - `callback`: The callback function as an instance of the `JNIMatchingStatusCallback` interface in Java/Kotlin.
/// - `on_close`: A Java/Kotlin `JNICallback` function interface to be called upon undeclaring the [ZAdvancedPublisher].
///
/// Safety:
/// - The function is marked as unsafe due to raw pointer manipulation and JNI interaction.
/// - It assumes that the provided [ZAdvancedPublisher] pointer is valid and has not been modified or freed.
/// - The [ZAdvancedPublisher] pointer remains valid and the ownership of the [ZAdvancedPublisher] is not transferred,
///   allowing safe usage of the [ZAdvancedPublisher] after this function call.
/// - The callback function passed as `callback` must be a valid instance of the `JNIMatchingStatusCallback` interface
///   in Java/Kotlin, matching the specified signature.
/// - The function may throw a JNI exception in case of failure, which should be handled by the caller.
///
#[cfg(feature = "zenoh-ext")]
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_io_zenoh_jni_JniZAdvancedPublisher_declareBackgroundMatchingListenerViaJNI(
    mut env: JNIEnv,
    _class: JClass,
    advanced_publisher_ptr: *const ZAdvancedPublisher,

    callback: JObject,
    on_close: JObject,
) {
    let advanced_publisher = OwnedObject::from_raw(advanced_publisher_ptr);

    || -> ZResult<()> {
        tracing::debug!(
            "Declaring background matching listener on '{}'...",
            advanced_publisher.key_expr()
        );

        advanced_publisher
            .matching_listener()
            .set_jni_matching_status_callback(&mut env, callback, on_close)?
            .background()
            .wait()
            .map_err(|err| zerror!("Unable to declare background matching listener: {}", err))?;

        tracing::debug!(
            "Background matching listener declared on '{}'...",
            advanced_publisher.key_expr()
        );
        Ok(())
    }()
    .unwrap_or_else(|err| {
        crate::generated::throw_ZError(&mut env, &err);
    });
}

/// Return the matching status of the [ZAdvancedPublisher].
///
/// Parameters:
/// - `env`: The JNI environment.
/// - `_class`: The JNI class.
/// - `advanced_publisher_ptr`: The raw pointer to an [ZAdvancedPublisher].
///
/// Returns:
/// - will return true if there exist Subscribers matching the Publisher's key expression and false otherwise.
///
/// Safety:
/// - The function is marked as unsafe due to raw pointer manipulation and JNI interaction.
/// - It assumes that the provided [ZAdvancedPublisher] pointer is valid and has not been modified or freed.
/// - The [ZAdvancedPublisher] pointer remains valid and the ownership of the [ZAdvancedPublisher] is not transferred,
///   allowing safe usage of the [ZAdvancedPublisher] after this function call.
/// - The function may throw a JNI exception in case of failure, which should be handled by the caller.
///
#[cfg(feature = "zenoh-ext")]
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_io_zenoh_jni_JniZAdvancedPublisher_getMatchingStatusViaJNI(
    mut env: JNIEnv,
    _class: JClass,
    advanced_publisher_ptr: *const ZAdvancedPublisher,
) -> jboolean {
    use zenoh_flat::errors::ZError;

    let advanced_publisher = OwnedObject::from_raw(advanced_publisher_ptr);
    advanced_publisher
        .matching_status()
        .wait()
        .map(|val| val.matching() as jboolean)
        .map_err(|e| zerror!(e.to_string()))
        .unwrap_or_else(|err: ZError| {
            crate::generated::throw_ZError(&mut env, &err);
            false as jboolean
        })
}

/// Performs a PUT operation on an [ZAdvancedPublisher] via JNI.
///
/// # Parameters
/// - `env`: The JNI environment pointer.
/// - `_class`: The Java class reference (unused).
/// - `payload`: The byte array to be published.
/// - `encoding_id`: The encoding ID of the payload.
/// - `encoding_schema`: Nullable encoding schema string of the payload.
/// - `attachment`: Nullble byte array for the attachment.
/// - `publisher_ptr`: The raw pointer to the [ZAdvancedPublisher].
///
/// # Safety
/// - This function is marked as unsafe due to raw pointer manipulation and JNI interaction.
/// - Assumes that the provided [ZAdvancedPublisher] pointer is valid and has not been modified or freed.
/// - The [ZAdvancedPublisher] pointer remains valid after this function call.
/// - May throw an exception in case of failure, which must be handled by the caller.
///
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_io_zenoh_jni_JniZAdvancedPublisher_putViaJNI(
    mut env: JNIEnv,
    _class: JClass,
    publisher_ptr: *const ZAdvancedPublisher<'static>,
    payload: JByteArray,
    encoding: JObject,
    attachment: /*nullable*/ JByteArray,
) {
    let publisher = OwnedObject::from_raw(publisher_ptr);
    let _ = || -> ZResult<()> {
        let payload = decode_byte_array(&env, &payload)?;
        let mut publication = publisher.put(payload);
        let encoding = decode_jni_encoding(&mut env, &encoding)?;
        publication = publication.encoding(encoding);
        if !attachment.is_null() {
            let attachment = decode_byte_array(&env, &attachment)?;
            publication = publication.attachment::<Vec<u8>>(attachment)
        };
        publication.wait().map_err(|err| zerror!(err))
    }()
    .map_err(|err| crate::generated::throw_ZError(&mut env, &err));
}

/// Performs a DELETE operation on an [ZAdvancedPublisher] via JNI.
///
/// # Parameters
/// - `env`: The JNI environment pointer.
/// - `_class`: The Java class reference (unused).
/// - `attachment`: Nullble byte array for the attachment.
/// - `publisher_ptr`: The raw pointer to the [ZAdvancedPublisher].
///
/// # Safety
/// - This function is marked as unsafe due to raw pointer manipulation and JNI interaction.
/// - Assumes that the provided [ZAdvancedPublisher] pointer is valid and has not been modified or freed.
/// - The [ZAdvancedPublisher] pointer remains valid after this function call.
/// - May throw an exception in case of failure, which must be handled by the caller.
///
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_io_zenoh_jni_JniZAdvancedPublisher_deleteViaJNI(
    mut env: JNIEnv,
    _class: JClass,
    publisher_ptr: *const ZAdvancedPublisher<'static>,
    attachment: /*nullable*/ JByteArray,
) {
    let publisher = OwnedObject::from_raw(publisher_ptr);
    let _ = || -> ZResult<()> {
        let mut delete = publisher.delete();
        if !attachment.is_null() {
            let attachment = decode_byte_array(&env, &attachment)?;
            delete = delete.attachment::<Vec<u8>>(attachment)
        };
        delete.wait().map_err(|err| zerror!(err))
    }()
    .map_err(|err| crate::generated::throw_ZError(&mut env, &err));
}

/// Frees the [ZAdvancedPublisher].
///
/// # Parameters:
/// - `_env`: The JNI environment.
/// - `_class`: The JNI class.
/// - `publisher_ptr`: The raw pointer to the [ZAdvancedPublisher].
///
/// # Safety:
/// - The function is marked as unsafe due to raw pointer manipulation.
/// - It assumes that the provided [ZAdvancedPublisher] pointer is valid and has not been modified or freed.
/// - After calling this function, the [ZAdvancedPublisher] pointer becomes invalid and should not be used anymore.
///
#[no_mangle]
#[allow(non_snake_case)]
pub(crate) unsafe extern "C" fn Java_io_zenoh_jni_JniZAdvancedPublisher_freePtrViaJNI(
    _env: JNIEnv,
    _: JClass,
    publisher_ptr: *const ZAdvancedPublisher,
) {
    drop(Box::from_raw(publisher_ptr as *mut ZAdvancedPublisher));
}
