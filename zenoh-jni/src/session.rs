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

use std::{ptr::null, sync::Arc, time::Duration};

use jni::{
    objects::{JClass, JString},
    sys::{jboolean, jint, jlong},
    JNIEnv,
};
use zenoh::{
    config::Config,
    key_expr::KeyExpr,
    pubsub::{Publisher, Subscriber},
    query::{Querier, Queryable},
    session::Session,
    Wait,
};

use crate::owned_object::OwnedObject;
#[cfg(feature = "zenoh-ext")]
use zenoh_ext::{
    AdvancedPublisher, AdvancedPublisherBuilderExt, AdvancedSubscriber, CacheConfig,
    MissDetectionConfig, RepliesConfig,
};

use crate::{errors::ZResult, key_expr::process_kotlin_key_expr, throw_exception, utils::*};

include!(concat!(env!("OUT_DIR"), "/zenoh_flat_jni.rs"));

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
