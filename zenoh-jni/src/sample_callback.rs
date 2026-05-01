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

use jni::{
    objects::{GlobalRef, JByteArray, JObject, JString, JValue},
    sys::{jint, jlong},
    JNIEnv,
};
use zenoh::{
    query::{Query, Reply, ReplyError, ReplyKeyExpr},
    sample::Sample,
    session::EntityGlobalId,
};

use crate::{errors::ZResult, utils::*};

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

pub(crate) unsafe fn process_kotlin_reply_callback(
    env: &mut JNIEnv,
    callback: &JObject,
) -> ZResult<impl Fn(Reply) + Send + Sync + 'static> {
    let java_vm = Arc::new(get_java_vm(env)?);
    let callback_global_ref = get_callback_global_ref(env, callback)?;

    Ok(move |reply: Reply| {
        || -> ZResult<()> {
            tracing::debug!("Receiving reply through JNI: {:?}", reply);
            let mut env = java_vm.attach_current_thread_as_daemon().map_err(|err| {
                zerror!("Unable to attach thread for GET query callback: {}", err)
            })?;
            match reply.result() {
                Ok(sample) => crate::sample_callback::on_reply_success(
                    &mut env,
                    reply.replier_id(),
                    sample,
                    &callback_global_ref,
                ),
                Err(error) => crate::sample_callback::on_reply_error(
                    &mut env,
                    reply.replier_id(),
                    error,
                    &callback_global_ref,
                ),
            }
        }()
        .unwrap_or_else(|err| tracing::error!("Error on get callback: {err}"));
    })
}

pub(crate) unsafe fn process_kotlin_query_callback(
    env: &mut JNIEnv,
    callback: &JObject,
) -> ZResult<impl Fn(Query) + Send + Sync + 'static> {
    let java_vm = Arc::new(get_java_vm(env)?);
    let callback_global_ref = get_callback_global_ref(env, callback)?;

    Ok(move |query: Query| {
        let env = match java_vm.attach_current_thread_as_daemon() {
            Ok(env) => env,
            Err(err) => {
                tracing::error!("Unable to attach thread for queryable callback: {}", err);
                return;
            }
        };

        tracing::debug!("Receiving query through JNI: {}", query.to_string());
        match on_query(env, query, &callback_global_ref) {
            Ok(_) => tracing::debug!("Queryable callback called successfully."),
            Err(err) => tracing::error!("Error calling queryable callback: {}", err),
        }
    })
}

/// Decoder for the zero-arg `impl Fn() + Send + Sync + 'static` callback used
/// by zenoh-flat for `on_close` parameters. The returned closure attaches the
/// JVM and invokes `Runnable.run()` on the Kotlin object. zenoh-flat is
/// responsible for binding it to the data closure's lifetime via its own
/// `CallOnDrop` so the on-close fires when the subscription/query is dropped.
pub(crate) unsafe fn process_kotlin_on_close_callback(
    env: &mut JNIEnv,
    on_close: &JObject,
) -> ZResult<impl Fn() + Send + Sync + 'static> {
    let java_vm = Arc::new(get_java_vm(env)?);
    let on_close_global_ref = get_callback_global_ref(env, on_close)?;

    Ok(move || {
        let mut env = match java_vm.attach_current_thread_as_daemon() {
            Ok(env) => env,
            Err(err) => {
                tracing::error!("Unable to attach thread for 'onClose' callback: {}", err);
                return;
            }
        };
        if let Err(err) = env.call_method(&on_close_global_ref, "run", "()V", &[]) {
            _ = env.exception_describe();
            tracing::error!("Error while running 'onClose' callback: {}", err);
        }
    })
}

fn on_query(mut env: JNIEnv, query: Query, callback_global_ref: &GlobalRef) -> ZResult<()> {
    let selector_params_jstr = env
        .new_string(query.parameters().to_string())
        .map(|value| env.auto_local(value))
        .map_err(|err| {
            zerror!(
                "Could not create a JString through JNI for the Query key expression. {}",
                err
            )
        })?;

    let (payload, encoding_id, encoding_schema) = if let Some(payload) = query.payload() {
        let encoding = query.encoding().unwrap(); //If there is payload, there is encoding.
        let encoding_id = encoding.id() as jint;
        let encoding_schema = encoding
            .schema()
            .map_or_else(
                || Ok(JString::default()),
                |schema| slice_to_java_string(&env, schema),
            )
            .map(|value| env.auto_local(value))?;
        let byte_array = bytes_to_java_array(&env, payload).map(|value| env.auto_local(value))?;
        (byte_array, encoding_id, encoding_schema)
    } else {
        (
            env.auto_local(JByteArray::default()),
            0,
            env.auto_local(JString::default()),
        )
    };

    let attachment_bytes = query
        .attachment()
        .map_or_else(
            || Ok(JByteArray::default()),
            |attachment| bytes_to_java_array(&env, attachment),
        )
        .map(|value| env.auto_local(value))
        .map_err(|err| zerror!("Error processing attachment of reply: {}.", err))?;

    let key_expr_str = env
        .new_string(query.key_expr().to_string())
        .map(|key_expr| env.auto_local(key_expr))
        .map_err(|err| {
            zerror!(
                "Could not create a JString through JNI for the Query key expression: {}.",
                err
            )
        })?;

    let accepts_replies: jint = match query.accepts_replies() {
        ReplyKeyExpr::MatchingQuery => 0,
        ReplyKeyExpr::Any => 1,
    };

    let query_ptr = Arc::into_raw(Arc::new(query));

    let result = env
        .call_method(
            callback_global_ref,
            "run",
            "(Ljava/lang/String;Ljava/lang/String;[BILjava/lang/String;[BJI)V",
            &[
                JValue::from(&key_expr_str),
                JValue::from(&selector_params_jstr),
                JValue::from(&payload),
                JValue::from(encoding_id),
                JValue::from(&encoding_schema),
                JValue::from(&attachment_bytes),
                JValue::from(query_ptr as jlong),
                JValue::from(accepts_replies),
            ],
        )
        .map(|_| ())
        .map_err(|err| {
            unsafe {
                Arc::from_raw(query_ptr);
            };
            _ = env.exception_describe();
            zerror!(err)
        });
    result
}
