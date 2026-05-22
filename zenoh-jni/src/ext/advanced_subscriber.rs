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

use jni::sys::jboolean;
use jni::{objects::JClass, JNIEnv};
use zenoh::handlers::{Callback as ZCallback, DefaultHandler as ZDefaultHandler};
use zenoh::pubsub::Subscriber as ZSubscriber;
use zenoh::sample::Sample as ZSample;
use zenoh_ext::SampleMissListener as ZSampleMissListener;
use zenoh_ext::{
    AdvancedSubscriber as ZAdvancedSubscriber, Miss as ZMiss,
    SampleMissListenerBuilder as ZSampleMissListenerBuilder,
};

use jni::objects::JObject;

use zenoh_flat::errors::ZResult;
use jni::objects::JValue;
use zenoh::Wait as ZWait;

use crate::generated::OwnedObject;

use crate::utils::{get_callback_global_ref, get_java_vm, load_on_close, wrap_with_on_close};
use std::ptr::null;


trait SetJniSampleMissListenerCallback {
    type WithCallback;

    unsafe fn set_jni_sample_miss_callback(
        self,
        env: &mut JNIEnv,
        callback: JObject,
        on_close: JObject,
    ) -> ZResult<Self::WithCallback>;
}

impl<'a> SetJniSampleMissListenerCallback for ZSampleMissListenerBuilder<'a, ZDefaultHandler> {
    type WithCallback = ZSampleMissListenerBuilder<'a, ZCallback<ZMiss>>;

    unsafe fn set_jni_sample_miss_callback(
        self,
        env: &mut JNIEnv,
        callback: JObject,
        on_close: JObject,
    ) -> ZResult<Self::WithCallback> {
        let java_vm = Arc::new(get_java_vm(env)?);
        let callback_global_ref = get_callback_global_ref(env, &callback)?;
        let on_close_global_ref = get_callback_global_ref(env, &on_close)?;
        let on_close = load_on_close(&java_vm, on_close_global_ref);

        let builder = self.callback(move |miss| {
            on_close.noop(); // Moves `on_close` inside the closure so it gets destroyed with the closure
            let _ = || -> ZResult<()> {
                let mut env = java_vm.attach_current_thread_as_daemon().map_err(|err| {
                    zerror!("Unable to attach thread for sample miss listener: {}", err)
                })?;

                let (zid_lower, zid_upper, eid) = {
                    let id = miss.source();

                    let zid = id.zid().to_le_bytes();
                    let zid_lower = i64::from_le_bytes(zid[0..8].try_into().unwrap());
                    let zid_upper = i64::from_le_bytes(zid[8..16].try_into().unwrap());

                    (zid_lower, zid_upper, id.eid())
                };
                let missed_count = miss.nb();

                env.call_method(
                    &callback_global_ref,
                    "run",
                    "(JJJJ)V",
                    &[
                        JValue::from(zid_lower),
                        JValue::from(zid_upper),
                        JValue::from(eid as i64),
                        JValue::from(missed_count as i64),
                    ],
                )
                .map_err(|err| zerror!(err))?;
                Ok(())
            }()
            .map_err(|err| tracing::error!("On sample miss listener callback error: {err}"));
        });
        Ok(builder)
    }
}

/// Declares a subscriber to detect matching publishers for an [ZAdvancedSubscriber] via JNI.
///
/// Parameters:
/// - `env`: The JNI environment.
/// - `_class`: The JNI class.
/// - `advanced_subscriber_ptr`: The raw pointer to the [ZAdvancedSubscriber].
/// - `callback`: The callback function as an instance of the `JNISampleCallback` interface in Java/Kotlin.
/// - `on_close`: A Java/Kotlin `JNICallback` function interface to be called upon closing the subscriber.
///
/// Returns:
/// - A raw pointer to the declared [ZSubscriber]. In case of failure, an exception is thrown and null is returned.
///
/// Safety:
/// - The function is marked as unsafe due to raw pointer manipulation and JNI interaction.
/// - It assumes that the provided [ZAdvancedSubscriber] pointer is valid and has not been modified or freed.
/// - The [ZAdvancedSubscriber] pointer remains valid and the ownership of the [ZAdvancedSubscriber] is not transferred,
///   allowing safe usage of the [ZAdvancedSubscriber] after this function call.
/// - The callback function passed as `callback` must be a valid instance of the `JNISampleCallback` interface
///   in Java/Kotlin, matching the specified signature.
/// - The function may throw a JNI exception in case of failure, which should be handled by the caller.
///
#[cfg(feature = "zenoh-ext")]
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_io_zenoh_jni_JniZAdvancedSubscriber_declareDetectPublishersSubscriberViaJNI(
    mut env: JNIEnv,
    _class: JClass,
    advanced_subscriber_ptr: *const ZAdvancedSubscriber<()>,
    history: jboolean,
    callback: JObject,
    on_close: JObject,
) -> *const ZSubscriber<()> {
    let advanced_subscriber = OwnedObject::from_raw(advanced_subscriber_ptr);

    || -> ZResult<*const ZSubscriber<()>> {
        tracing::debug!(
            "Declaring detect publishers subscriber on '{}'...",
            advanced_subscriber.key_expr()
        );

        // `process_kotlin_SampleCallback_callback` is the auto-generated callback
        // dispatcher — it returns `Result<_, JniBindingError>` (the
        // framework error type). The enclosing closure is in a `ZResult`
        // pipeline; convert via `Display` since the orphan rule blocks
        // a `From<JniBindingError> for ZError` impl in any crate.
        let cb_flat = crate::generated::process_kotlin_SampleCallback_callback(&mut env, &callback)
            .map_err(|e| zerror!("Sample callback: {}", e))?;
        let cb = move |zsample: ZSample| {
            cb_flat((&zsample).into());
        };
        let cb = wrap_with_on_close(&mut env, on_close, cb)?;
        let detect_publishers_subscriber = advanced_subscriber
            .detect_publishers()
            .history(history != 0)
            .callback(cb)
            .wait()
            .map_err(|err| zerror!("Unable to declare detect publishers subscriber: {}", err))?;

        tracing::debug!(
            "Detect publishers subscriber declared on '{}'...",
            advanced_subscriber.key_expr()
        );
        Ok(Box::into_raw(Box::new(detect_publishers_subscriber)) as *const ZSubscriber<()>)
    }()
    .unwrap_or_else(|err| {
        crate::generated::throw_ZError(&mut env, &err);
        null()
    })
}

/// Declares a background subscriber to detect matching publishers for an [ZAdvancedSubscriber] via JNI.
///
/// Parameters:
/// - `env`: The JNI environment.
/// - `_class`: The JNI class.
/// - `advanced_subscriber_ptr`: The raw pointer to the [ZAdvancedSubscriber].
/// - `callback`: The callback function as an instance of the `JNISampleCallback` interface in Java/Kotlin.
/// - `on_close`: A Java/Kotlin `JNICallback` function interface to be called upon closing the subscriber.
///
/// Safety:
/// - The function is marked as unsafe due to raw pointer manipulation and JNI interaction.
/// - It assumes that the provided [ZAdvancedSubscriber] pointer is valid and has not been modified or freed.
/// - The [ZAdvancedSubscriber] pointer remains valid and the ownership of the [ZAdvancedSubscriber] is not transferred,
///   allowing safe usage of the [ZAdvancedSubscriber] after this function call.
/// - The callback function passed as `callback` must be a valid instance of the `JNISampleCallback` interface
///   in Java/Kotlin, matching the specified signature.
/// - The function may throw a JNI exception in case of failure, which should be handled by the caller.
///
#[cfg(feature = "zenoh-ext")]
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_io_zenoh_jni_JniZAdvancedSubscriber_declareBackgroundDetectPublishersSubscriberViaJNI(
    mut env: JNIEnv,
    _class: JClass,
    advanced_subscriber_ptr: *const ZAdvancedSubscriber<()>,
    history: jboolean,
    callback: JObject,
    on_close: JObject,
) {
    let advanced_subscriber = OwnedObject::from_raw(advanced_subscriber_ptr);

    || -> ZResult<()> {
        tracing::debug!(
            "Declaring background detect publishers subscriber on '{}'...",
            advanced_subscriber.key_expr()
        );

        // `process_kotlin_SampleCallback_callback` is the auto-generated callback
        // dispatcher — it returns `Result<_, JniBindingError>` (the
        // framework error type). The enclosing closure is in a `ZResult`
        // pipeline; convert via `Display` since the orphan rule blocks
        // a `From<JniBindingError> for ZError` impl in any crate.
        let cb_flat = crate::generated::process_kotlin_SampleCallback_callback(&mut env, &callback)
            .map_err(|e| zerror!("Sample callback: {}", e))?;
        let cb = move |zsample: ZSample| {
            cb_flat((&zsample).into());
        };
        let cb = wrap_with_on_close(&mut env, on_close, cb)?;
        advanced_subscriber
            .detect_publishers()
            .history(history != 0)
            .callback(cb)
            .background()
            .wait()
            .map_err(|err| {
                zerror!(
                    "Unable to declare background detect publishers subscriber: {}",
                    err
                )
            })?;

        tracing::debug!(
            "Background detect publishers subscriber declared on '{}'...",
            advanced_subscriber.key_expr()
        );
        Ok(())
    }()
    .unwrap_or_else(|err| {
        crate::generated::throw_ZError(&mut env, &err);
    });
}

/// Declares a [ZSampleMissListener] to detect missed samples for an [ZAdvancedSubscriber] via JNI.
///
/// Parameters:
/// - `env`: The JNI environment.
/// - `_class`: The JNI class.
/// - `advanced_subscriber_ptr`: The raw pointer to the [ZAdvancedSubscriber].
/// - `callback`: The callback function as an instance of the `JNISampleMissedCallback` interface in Java/Kotlin.
/// - `on_close`: A Java/Kotlin `JNICallback` function interface to be called upon closing the subscriber.
///
/// Returns:
/// - A raw pointer to the declared [ZSampleMissListener]. In case of failure, an exception is thrown and null is returned.
///
/// Safety:
/// - The function is marked as unsafe due to raw pointer manipulation and JNI interaction.
/// - It assumes that the provided [ZAdvancedSubscriber] pointer is valid and has not been modified or freed.
/// - The [ZAdvancedSubscriber] pointer remains valid and the ownership of the [ZAdvancedSubscriber] is not transferred,
///   allowing safe usage of the [ZAdvancedSubscriber] after this function call.
/// - The callback function passed as `callback` must be a valid instance of the `JNISampleMissedCallback` interface
///   in Java/Kotlin, matching the specified signature.
/// - The function may throw a JNI exception in case of failure, which should be handled by the caller.
///
#[cfg(feature = "zenoh-ext")]
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_io_zenoh_jni_JniZAdvancedSubscriber_declareSampleMissListenerViaJNI(
    mut env: JNIEnv,
    _class: JClass,
    advanced_subscriber_ptr: *const ZAdvancedSubscriber<()>,

    callback: JObject,
    on_close: JObject,
) -> *const ZSampleMissListener<()> {
    let advanced_subscriber = OwnedObject::from_raw(advanced_subscriber_ptr);

    || -> ZResult<*const ZSampleMissListener<()>> {
        tracing::debug!(
            "Declaring sample miss listener on '{}'...",
            advanced_subscriber.key_expr()
        );

        let result = advanced_subscriber
            .sample_miss_listener()
            .set_jni_sample_miss_callback(&mut env, callback, on_close)?
            .wait();

        let sample_miss_listener =
            result.map_err(|err| zerror!("Unable to declare sample miss listener: {}", err))?;

        tracing::debug!(
            "Matching listener declared on '{}'...",
            advanced_subscriber.key_expr()
        );
        Ok(Box::into_raw(Box::new(sample_miss_listener)) as *const ZSampleMissListener<()>)
    }()
    .unwrap_or_else(|err| {
        crate::generated::throw_ZError(&mut env, &err);
        null()
    })
}

/// Declare a background sample miss listener for [ZAdvancedSubscriber] via JNI.
/// Register the listener callback to be run in background until the [ZAdvancedSubscriber] is undeclared.
///
/// Parameters:
/// - `env`: The JNI environment.
/// - `_class`: The JNI class.
/// - `advanced_subscriber_ptr`: The raw pointer to an [ZAdvancedSubscriber].
/// - `callback`: The callback function as an instance of the `JNISampleMissedCallback` interface in Java/Kotlin.
/// - `on_close`: A Java/Kotlin `JNICallback` function interface to be called upon undeclaring the [ZAdvancedSubscriber].
///
/// Safety:
/// - The function is marked as unsafe due to raw pointer manipulation and JNI interaction.
/// - It assumes that the provided [ZAdvancedSubscriber] pointer is valid and has not been modified or freed.
/// - The [ZAdvancedSubscriber] pointer remains valid and the ownership of the [ZAdvancedSubscriber] is not transferred,
///   allowing safe usage of the [ZAdvancedSubscriber] after this function call.
/// - The callback function passed as `callback` must be a valid instance of the `JNISampleMissedCallback` interface
///   in Java/Kotlin, matching the specified signature.
/// - The function may throw a JNI exception in case of failure, which should be handled by the caller.
///
#[cfg(feature = "zenoh-ext")]
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_io_zenoh_jni_JniZAdvancedSubscriber_declareBackgroundSampleMissListenerViaJNI(
    mut env: JNIEnv,
    _class: JClass,
    advanced_subscriber_ptr: *const ZAdvancedSubscriber<()>,

    callback: JObject,
    on_close: JObject,
) {
    let advanced_subscriber = OwnedObject::from_raw(advanced_subscriber_ptr);

    || -> ZResult<()> {
        tracing::debug!(
            "Declaring background sample miss listener on '{}'...",
            advanced_subscriber.key_expr()
        );

        advanced_subscriber
            .sample_miss_listener()
            .set_jni_sample_miss_callback(&mut env, callback, on_close)?
            .background()
            .wait()
            .map_err(|err| zerror!("Unable to declare background sample miss listener: {}", err))?;

        tracing::debug!(
            "Background sample miss listener declared on '{}'...",
            advanced_subscriber.key_expr()
        );
        Ok(())
    }()
    .unwrap_or_else(|err| {
        crate::generated::throw_ZError(&mut env, &err);
    })
}

/// Frees the [ZAdvancedSubscriber].
///
/// # Parameters:
/// - `_env`: The JNI environment.
/// - `_class`: The JNI class.
/// - `subscriber_ptr`: The raw pointer to the [ZAdvancedSubscriber].
///
/// # Safety:
/// - The function is marked as unsafe due to raw pointer manipulation.
/// - It assumes that the provided [ZAdvancedSubscriber] pointer is valid and has not been modified or freed.
/// - The function takes ownership of the raw pointer and releases the associated memory.
/// - After calling this function, the [ZAdvancedSubscriber] pointer becomes invalid and should not be used anymore.
///
#[no_mangle]
#[allow(non_snake_case)]
pub(crate) unsafe extern "C" fn Java_io_zenoh_jni_JniZAdvancedSubscriber_freePtrViaJNI(
    _env: JNIEnv,
    _: JClass,
    subscriber_ptr: *const ZAdvancedSubscriber<()>,
) {
    drop(Box::from_raw(subscriber_ptr as *mut ZAdvancedSubscriber<()>));
}
