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

use std::{ops::Deref, ptr::null, sync::Arc, time::Duration};

use jni::{
    objects::{GlobalRef, JByteArray, JClass, JList, JObject, JString, JValue},
    sys::{jboolean, jbyteArray, jint, jlong, jobject},
    JNIEnv,
};
use zenoh::{
    config::Config,
    key_expr::KeyExpr,
    pubsub::{Publisher, Subscriber},
    query::{Querier, Queryable, ReplyError},
    sample::Sample,
    session::{EntityGlobalId, Session, ZenohId},
    Wait,
};

use crate::owned_object::OwnedObject;
use crate::sample_callback::{process_kotlin_reply_callback, process_kotlin_sample_callback};
#[cfg(feature = "zenoh-ext")]
use jni::sys::jdouble;
#[cfg(feature = "zenoh-ext")]
use zenoh_ext::{
    AdvancedPublisher, AdvancedPublisherBuilderExt, AdvancedSubscriber,
    AdvancedSubscriberBuilderExt, CacheConfig, HistoryConfig, MissDetectionConfig, RecoveryConfig,
    RepliesConfig,
};

use crate::{
    errors::ZResult, key_expr::process_kotlin_key_expr, throw_exception, utils::*,
};

include!(concat!(env!("OUT_DIR"), "/zenoh_flat_jni.rs"));







/// Undeclare a [KeyExpr] through a [Session] via JNI.
///
/// The key expression must have been previously declared on the specified session, otherwise an
/// exception is thrown.
///
/// This functions frees the key expression pointer provided.
///
/// # Parameters:
/// - `env`: The JNI environment.
/// - `_class`: The JNI class.
/// - `session_ptr`: A raw pointer to the Zenoh [Session] from which to undeclare the key expression.
/// - `key_expr_ptr`: A raw pointer to the [KeyExpr] to undeclare.
///
/// # Safety:
/// - The function is marked as unsafe due to raw pointer manipulation and JNI interaction.
/// - It assumes that the provided session and keyexpr pointers are valid and have not been modified or freed.
/// - The session pointer remains valid after this function call.
/// - The key expression pointer is voided after this function call.
/// - The function may throw an exception in case of failure, which should be handled by the caller.
///
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_io_zenoh_jni_JNISession_undeclareKeyExprViaJNI(
    mut env: JNIEnv,
    _class: JClass,
    session_ptr: *const Session,
    key_expr_ptr: *const KeyExpr<'static>,
) {
    let session = OwnedObject::from_raw(session_ptr);
    let key_expr = Arc::from_raw(key_expr_ptr);
    let key_expr_clone = key_expr.deref().clone();
    match session.undeclare(key_expr_clone).wait() {
        Ok(_) => {}
        Err(err) => {
            throw_exception!(
                env,
                zerror!("Unable to declare key expression '{}': {}", key_expr, err)
            );
        }
    }
    // `key_expr` is intentionally left to be freed by Rust
}

/// Performs a `get` operation in the Zenoh session via JNI with Value.
///
/// # Parameters:
/// - `env`: The JNI environment.
/// - `_class`: The JNI class.
/// - `key_expr_ptr`: Raw pointer to a declared [KeyExpr] to be used for the query. May be null in case
///   of using a non declared key expression, in which case the `key_expr_str` parameter will be used instead.
/// - `key_expr_str`: String representation of the key expression to be used to declare the query. It is not
///   considered if a `key_expr_ptr` is provided.
/// - `selector_params`: Optional parameters of the selector.
/// - `session_ptr`: A raw pointer to the Zenoh [Session].
/// - `callback`: A Java/Kotlin callback to be called upon receiving a reply.
/// - `on_close`: A Java/Kotlin `JNIOnCloseCallback` function interface to be called when no more replies will be received.
/// - `timeout_ms`: The timeout in milliseconds.
/// - `target`: The query target as the ordinal of the enum.
/// - `consolidation`: The consolidation mode as the ordinal of the enum.
/// - `attachment`: An optional attachment encoded into a byte array.
/// - `payload`: Optional payload for the query.
/// - `encoding_id`: The encoding of the payload.
/// - `encoding_schema`: The encoding schema of the payload, may be null.
/// - `congestion_control`: The ordinal value of the congestion control enum value.
/// - `priority`: The ordinal value of the priority enum value.
/// - `is_express`: The boolean express value of the QoS provided.
///
/// Safety:
/// - The function is marked as unsafe due to raw pointer manipulation and JNI interaction.
/// - It assumes that the provided session pointer is valid and has not been modified or freed.
/// - The session pointer remains valid and the ownership of the session is not transferred,
///   allowing safe usage of the session after this function call.
/// - The function may throw a JNI exception in case of failure, which should be handled by the caller.
///
/// Throws:
/// - An exception in case of failure handling the query.
///
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_io_zenoh_jni_JNISession_getViaJNI(
    mut env: JNIEnv,
    _class: JClass,
    session_ptr: *const Session,
    key_expr_ptr: /*nullable*/ *const KeyExpr<'static>,
    key_expr_str: JString,
    selector_params: /*nullable*/ JString,
    callback: JObject,
    on_close: JObject,
    timeout_ms: jlong,
    target: jint,
    consolidation: jint,
    attachment: /*nullable*/ JByteArray,
    payload: /*nullable*/ JByteArray,
    encoding_id: jint,
    encoding_schema: /*nullable*/ JString,
    congestion_control: jint,
    priority: jint,
    is_express: jboolean,
    accept_replies: jint,
) {
    let session = OwnedObject::from_raw(session_ptr);
    let _ = || -> ZResult<()> {
        let key_expr = process_kotlin_key_expr(&mut env, &key_expr_str, key_expr_ptr)?;
        let reply_callback = process_kotlin_reply_callback(&mut env, callback, on_close)?;
        let query_target = decode_query_target(target)?;
        let consolidation = decode_consolidation(consolidation)?;
        let timeout = Duration::from_millis(timeout_ms as u64);
        let congestion_control = decode_congestion_control(congestion_control)?;
        let priority = decode_priority(priority)?;
        let reply_key_expr = decode_reply_key_expr(accept_replies)?;
        let selector_params = if selector_params.is_null() {
            String::new()
        } else {
            decode_string(&mut env, &selector_params)?
        };

        let payload = if !payload.is_null() {
            Some(decode_byte_array(&env, payload)?)
        } else {
            None
        };
        let encoding = if payload.is_some() {
            Some(decode_encoding(&mut env, encoding_id, &encoding_schema)?)
        } else {
            None
        };
        let attachment = if !attachment.is_null() {
            Some(decode_byte_array(&env, attachment)?)
        } else {
            None
        };

        zenoh_flat::session::get(
            &session,
            key_expr,
            selector_params,
            reply_callback,
            query_target,
            consolidation,
            congestion_control,
            priority,
            is_express != 0,
            timeout,
            reply_key_expr,
            payload,
            encoding,
            attachment,
        )
    }()
    .map_err(|err| throw_exception!(env, err));
}

pub(crate) fn on_reply_success(
    env: &mut JNIEnv,
    replier_id: Option<EntityGlobalId>,
    sample: &Sample,
    callback_global_ref: &GlobalRef,
) -> ZResult<()> {
    let zenoh_id = replier_id
        .map_or_else(
            || Ok(JByteArray::default()),
            |replier_id| {
                env.byte_array_from_slice(&replier_id.zid().to_le_bytes())
                    .map_err(|err| zerror!(err))
            },
        )
        .map(|value| env.auto_local(value))?;
    let eid = replier_id.map_or_else(|| 0, |replier_id| replier_id.eid() as jint);

    let byte_array =
        bytes_to_java_array(env, sample.payload()).map(|value| env.auto_local(value))?;
    let encoding: jint = sample.encoding().id() as jint;
    let encoding_schema = sample
        .encoding()
        .schema()
        .map_or_else(
            || Ok(JString::default()),
            |schema| slice_to_java_string(env, schema),
        )
        .map(|value| env.auto_local(value))?;
    let kind = sample.kind() as jint;

    let (timestamp, is_valid) = sample
        .timestamp()
        .map(|timestamp| (timestamp.get_time().as_u64(), true))
        .unwrap_or((0, false));

    let attachment_bytes = sample
        .attachment()
        .map_or_else(
            || Ok(JByteArray::default()),
            |attachment| bytes_to_java_array(env, attachment),
        )
        .map(|value| env.auto_local(value))
        .map_err(|err| zerror!("Error processing attachment of reply: {}.", err))?;

    let key_expr_str = env
        .new_string(sample.key_expr().to_string())
        .map(|value| env.auto_local(value))
        .map_err(|err| {
            zerror!(
                "Could not create a JString through JNI for the Sample key expression. {}",
                err
            )
        })?;

    let express = sample.express();
    let priority = sample.priority() as jint;
    let cc = sample.congestion_control() as jint;

    let result = match env.call_method(
        callback_global_ref,
        "run",
        "([BIZLjava/lang/String;[BILjava/lang/String;IJZ[BZII)V",
        &[
            JValue::from(&zenoh_id),
            JValue::from(eid),
            JValue::from(true),
            JValue::from(&key_expr_str),
            JValue::from(&byte_array),
            JValue::from(encoding),
            JValue::from(&encoding_schema),
            JValue::from(kind),
            JValue::from(timestamp as i64),
            JValue::from(is_valid),
            JValue::from(&attachment_bytes),
            JValue::from(express),
            JValue::from(priority),
            JValue::from(cc),
        ],
    ) {
        Ok(_) => Ok(()),
        Err(err) => {
            _ = env.exception_describe();
            Err(zerror!("On GET callback error: {}", err))
        }
    };
    result
}

pub(crate) fn on_reply_error(
    env: &mut JNIEnv,
    replier_id: Option<EntityGlobalId>,
    reply_error: &ReplyError,
    callback_global_ref: &GlobalRef,
) -> ZResult<()> {
    let zenoh_id = replier_id
        .map_or_else(
            || Ok(JByteArray::default()),
            |replier_id| {
                env.byte_array_from_slice(&replier_id.zid().to_le_bytes())
                    .map_err(|err| zerror!(err))
            },
        )
        .map(|value| env.auto_local(value))?;
    let eid = replier_id.map_or_else(|| 0, |replier_id| replier_id.eid() as jint);

    let payload =
        bytes_to_java_array(env, reply_error.payload()).map(|value| env.auto_local(value))?;
    let encoding_id: jint = reply_error.encoding().id() as jint;
    let encoding_schema = reply_error
        .encoding()
        .schema()
        .map_or_else(
            || Ok(JString::default()),
            |schema| slice_to_java_string(env, schema),
        )
        .map(|value| env.auto_local(value))?;
    let result = match env.call_method(
        callback_global_ref,
        "run",
        "([BIZLjava/lang/String;[BILjava/lang/String;IJZ[BZII)V",
        &[
            JValue::from(&zenoh_id),
            JValue::from(eid),
            JValue::from(false),
            JValue::from(&JString::default()),
            JValue::from(&payload),
            JValue::from(encoding_id),
            JValue::from(&encoding_schema),
            // The remaining parameters aren't used in case of replying error, so we set them to default.
            JValue::from(0 as jint),
            JValue::from(0_i64),
            JValue::from(false),
            JValue::from(&JByteArray::default()),
            JValue::from(false),
            JValue::from(0 as jint),
            JValue::from(0 as jint),
        ],
    ) {
        Ok(_) => Ok(()),
        Err(err) => {
            _ = env.exception_describe();
            Err(zerror!("On GET callback error: {}", err))
        }
    };
    result
}

/// Returns a list of zenoh ids as byte arrays corresponding to the peers connected to the session provided.
///
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_io_zenoh_jni_JNISession_getPeersZidViaJNI(
    mut env: JNIEnv,
    _class: JClass,
    session_ptr: *const Session,
) -> jobject {
    let session = OwnedObject::from_raw(session_ptr);
    {
        let peers_zid = session.info().peers_zid().wait();
        let ids = peers_zid.collect::<Vec<ZenohId>>();
        ids_to_java_list(&mut env, ids).map_err(|err| zerror!(err))
    }
    .unwrap_or_else(|err| {
        throw_exception!(env, err);
        JObject::default().as_raw()
    })
}

/// Returns a list of zenoh ids as byte arrays corresponding to the routers connected to the session provided.
///
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_io_zenoh_jni_JNISession_getRoutersZidViaJNI(
    mut env: JNIEnv,
    _class: JClass,
    session_ptr: *const Session,
) -> jobject {
    let session = OwnedObject::from_raw(session_ptr);
    {
        let peers_zid = session.info().routers_zid().wait();
        let ids = peers_zid.collect::<Vec<ZenohId>>();
        ids_to_java_list(&mut env, ids).map_err(|err| zerror!(err))
    }
    .unwrap_or_else(|err| {
        throw_exception!(env, err);
        JObject::default().as_raw()
    })
}

/// Returns the Zenoh ID as a byte array of the session.
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_io_zenoh_jni_JNISession_getZidViaJNI(
    mut env: JNIEnv,
    _class: JClass,
    session_ptr: *const Session,
) -> jbyteArray {
    let session = OwnedObject::from_raw(session_ptr);
    {
        let zid = session.info().zid().wait();
        env.byte_array_from_slice(&zid.to_le_bytes())
            .map(|x| x.as_raw())
            .map_err(|err| zerror!(err))
    }
    .unwrap_or_else(|err| {
        throw_exception!(env, err);
        JByteArray::default().as_raw()
    })
}

fn ids_to_java_list(env: &mut JNIEnv, ids: Vec<ZenohId>) -> jni::errors::Result<jobject> {
    let array_list = env.new_object("java/util/ArrayList", "()V", &[])?;
    let jlist = JList::from_env(env, &array_list)?;
    for id in ids {
        let value = &mut env.byte_array_from_slice(&id.to_le_bytes())?;
        jlist.add(env, value)?;
    }
    Ok(array_list.as_raw())
}

/// Declare an advanced Zenoh subscriber via JNI.
///
/// # Parameters:
/// - `env`: The JNI environment.
/// - `_class`: The JNI class.
/// - `key_expr_ptr`: Raw pointer to the [KeyExpr], may be null.
/// - `key_expr_str`: String representation of the [KeyExpr].
/// - `history_config_enabled`: Whether history config is enabled.
/// - `history_detect_late_publishers`: Whether to detect late publishers in history.
/// - `history_max_samples`: Max samples in history (<=0 means unlimited).
/// - `history_max_age_seconds`: Max age in seconds for history (<=0.0 means unlimited).
/// - `recovery_config_enabled`: Whether recovery config is enabled.
/// - `recovery_config_is_heartbeat`: Whether to use heartbeat recovery.
/// - `recovery_query_period_ms`: Query period for periodic recovery in milliseconds.
/// - `subscriber_detection`: Allow this subscriber to be detected through liveliness.
/// - `session_ptr`: The raw pointer to the Zenoh session.
/// - `callback`: The callback function as an instance of the `JNISubscriberCallback` interface.
/// - `on_close`: A `JNIOnCloseCallback` to be called upon closing the subscriber.
///
/// Returns:
/// - A raw pointer to the declared [AdvancedSubscriber]. In case of failure, an exception is thrown and null is returned.
///
/// Safety:
/// - The function is marked as unsafe due to raw pointer manipulation and JNI interaction.
///
#[cfg(feature = "zenoh-ext")]
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_io_zenoh_jni_JNISession_declareAdvancedSubscriberViaJNI(
    mut env: JNIEnv,
    _class: JClass,
    session_ptr: *const Session,
    key_expr_ptr: /*nullable*/ *const KeyExpr<'static>,
    key_expr_str: JString,
    // HistoryConfig
    history_config_enabled: jboolean,
    history_detect_late_publishers: jboolean,
    history_max_samples: jlong,
    history_max_age_seconds: jdouble,
    // RecoveryConfig
    recovery_config_enabled: jboolean,
    recovery_config_is_heartbeat: jboolean,
    recovery_query_period_ms: jlong,

    subscriber_detection: jboolean,

    callback: JObject,
    on_close: JObject,
) -> *const AdvancedSubscriber<()> {
    let session = OwnedObject::from_raw(session_ptr);
    || -> ZResult<*const AdvancedSubscriber<()>> {
        let key_expr = process_kotlin_key_expr(&mut env, &key_expr_str, key_expr_ptr)?;
        tracing::debug!("Declaring advanced subscriber on '{}'...", key_expr);
        let mut builder = session
            .declare_subscriber(key_expr.to_owned())
            .callback(process_kotlin_sample_callback(&mut env, callback, on_close)?)
            .advanced();
        tracing::debug!("Advanced subscriber declared on '{}'.", key_expr);

        if history_config_enabled != 0 {
            let mut history = match history_detect_late_publishers != 0 {
                true => HistoryConfig::default().detect_late_publishers(),
                false => HistoryConfig::default(),
            };

            if history_max_samples > 0 {
                history = history.max_samples(
                    history_max_samples
                        .try_into()
                        .map_err(|e: std::num::TryFromIntError| zerror!(e.to_string()))?,
                );
            }

            if history_max_age_seconds > 0.0 {
                history = history.max_age(history_max_age_seconds);
            }

            builder = builder.history(history);
        }

        if recovery_config_enabled != 0 {
            let recovery = if recovery_config_is_heartbeat != 0 {
                RecoveryConfig::default().heartbeat()
            } else {
                let dur = Duration::from_millis(
                    recovery_query_period_ms
                        .try_into()
                        .map_err(|e: std::num::TryFromIntError| zerror!(e.to_string()))?,
                );
                RecoveryConfig::default().periodic_queries(dur)
            };
            builder = builder.recovery(recovery);
        }

        if subscriber_detection != 0 {
            builder = builder.subscriber_detection();
        }

        builder
            .wait()
            .map(|s| Arc::into_raw(Arc::new(s)))
            .map_err(|err| zerror!("Unable to declare advanced subscriber: {}", err))
    }()
    .unwrap_or_else(|err| {
        throw_exception!(env, err);
        null()
    })
}

/// Declare an advanced Zenoh publisher via JNI.
///
/// # Parameters:
/// - `env`: The JNI environment.
/// - `_class`: The JNI class.
/// - `key_expr_ptr`: Raw pointer to the [KeyExpr] to be used for the publisher, may be null.
/// - `key_expr_str`: String representation of the [KeyExpr].
/// - `session_ptr`: Raw pointer to the Zenoh [Session] to be used for the publisher.
/// - `congestion_control`: The [CongestionControl] configuration as an ordinal.
/// - `priority`: The [Priority] configuration as an ordinal.
/// - `is_express`: The express config of the publisher.
/// - `reliability`: The reliability value as an ordinal.
/// - `cache_enabled`: If true, attach a cache to the [AdvancedPublisher].
/// - `cache_max_samples`: How many samples to keep for each resource.
/// - `cache_replies_priority`: The [Priority] for cache replies as an ordinal.
/// - `cache_replies_congestion_control`: The [CongestionControl] for cache replies as an ordinal.
/// - `cache_replies_is_express`: The express config for cache replies.
/// - `sample_miss_detection_enabled`: Enables sample miss detection functionality.
/// - `sample_miss_detection_enable_heartbeat`: Use heartbeat for miss detection.
/// - `sample_miss_detection_heartbeat_ms`: Heartbeat period in milliseconds.
/// - `sample_miss_detection_heartbeat_is_sporadic`: Whether heartbeat is sporadic.
/// - `publisher_detection`: Enables publisher detection.
///
/// # Returns:
/// - A raw pointer to the declared [AdvancedPublisher] or null in case of failure.
///
/// # Safety:
/// - The function is marked as unsafe due to raw pointer manipulation and JNI interaction.
///
#[cfg(feature = "zenoh-ext")]
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_io_zenoh_jni_JNISession_declareAdvancedPublisherViaJNI(
    mut env: JNIEnv,
    _class: JClass,
    session_ptr: *const Session,
    key_expr_ptr: /*nullable*/ *const KeyExpr<'static>,
    key_expr_str: JString,
    congestion_control: jint,
    priority: jint,
    is_express: jboolean,
    reliability: jint,
    // CacheConfig
    cache_enabled: jboolean,
    cache_max_samples: jlong,
    cache_replies_priority: jint,
    cache_replies_congestion_control: jint,
    cache_replies_is_express: jboolean,
    // MissDetectionConfig
    sample_miss_detection_enabled: jboolean,
    sample_miss_detection_enable_heartbeat: jboolean,
    sample_miss_detection_heartbeat_ms: jlong,
    sample_miss_detection_heartbeat_is_sporadic: jboolean,

    publisher_detection: jboolean,
) -> *const AdvancedPublisher<'static> {
    let session = OwnedObject::from_raw(session_ptr);
    let publisher_ptr = || -> ZResult<*const AdvancedPublisher<'static>> {
        let key_expr = process_kotlin_key_expr(&mut env, &key_expr_str, key_expr_ptr)?;
        let congestion_control = decode_congestion_control(congestion_control)?;
        let priority = decode_priority(priority)?;
        let reliability = decode_reliability(reliability)?;
        let mut builder = session
            .declare_publisher(key_expr)
            .congestion_control(congestion_control)
            .priority(priority)
            .express(is_express != 0)
            .reliability(reliability)
            .advanced();

        // fill CacheConfig
        if cache_enabled != 0 {
            let cache_congestion_control =
                decode_congestion_control(cache_replies_congestion_control)?;

            let cache_priority = decode_priority(cache_replies_priority)?;

            let replies_config = RepliesConfig::default()
                .priority(cache_priority)
                .congestion_control(cache_congestion_control)
                .express(cache_replies_is_express != 0);

            let cache_config = CacheConfig::default()
                .max_samples(
                    cache_max_samples
                        .try_into()
                        .map_err(|e: std::num::TryFromIntError| zerror!(e.to_string()))?,
                )
                .replies_config(replies_config);

            builder = builder.cache(cache_config);
        }

        // fill MissDetectionConfig
        if sample_miss_detection_enabled != 0 {
            let miss_detection_config = {
                let mut result = MissDetectionConfig::default();
                if sample_miss_detection_enable_heartbeat != 0 {
                    let duration = Duration::from_millis(
                        sample_miss_detection_heartbeat_ms
                            .try_into()
                            .map_err(|e: std::num::TryFromIntError| zerror!(e.to_string()))?,
                    );

                    result = match sample_miss_detection_heartbeat_is_sporadic != 0 {
                        true => result.sporadic_heartbeat(duration),
                        false => result.heartbeat(duration),
                    };
                }
                result
            };
            builder = builder.sample_miss_detection(miss_detection_config);
        }

        if publisher_detection != 0 {
            builder = builder.publisher_detection();
        }

        let result = builder.wait();
        match result {
            Ok(publisher) => Ok(Arc::into_raw(Arc::new(publisher))),
            Err(err) => Err(zerror!(err)),
        }
    }()
    .unwrap_or_else(|err| {
        throw_exception!(env, err);
        null()
    });
    publisher_ptr
}
