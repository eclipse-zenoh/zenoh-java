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

package io.zenoh.jni

import io.zenoh.bytes.Encoding
import io.zenoh.bytes.into
import io.zenoh.config.ZenohId
import io.zenoh.exceptions.ZError
import io.zenoh.handlers.Callback
import io.zenoh.jni.callbacks.JNIGetCallback
import io.zenoh.jni.callbacks.JNIOnCloseCallback
import io.zenoh.jni.callbacks.JNISubscriberCallback
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.liveliness.LivelinessToken
import io.zenoh.pubsub.CallbackSubscriber
import io.zenoh.pubsub.HandlerSubscriber
import io.zenoh.qos.CongestionControl
import io.zenoh.qos.Priority
import io.zenoh.qos.QoS
import io.zenoh.query.Reply
import io.zenoh.sample.Sample
import io.zenoh.sample.SampleKind
import org.apache.commons.net.ntp.TimeStamp
import java.time.Duration

internal object JNILiveliness {

    @Throws(ZError::class)
    fun <R> get(
        jniSession: JNISession,
        keyExpr: KeyExpr,
        callback: Callback<Reply>,
        receiver: R,
        timeout: Duration,
        onClose: Runnable
    ): R {
        val getCallback = JNIGetCallback {
                replierId: ByteArray?,
                success: Boolean,
                keyExpr2: String?,
                payload: ByteArray,
                encodingId: Int,
                encodingSchema: String?,
                kind: Int,
                timestampNTP64: Long,
                timestampIsValid: Boolean,
                attachmentBytes: ByteArray?,
                express: Boolean,
                priority: Int,
                congestionControl: Int,
            ->
            val reply: Reply
            if (success) {
                val timestamp = if (timestampIsValid) TimeStamp(timestampNTP64) else null
                val sample = Sample(
                    KeyExpr(keyExpr2!!, null),
                    payload.into(),
                    Encoding(encodingId, schema = encodingSchema),
                    SampleKind.fromInt(kind),
                    timestamp,
                    QoS(CongestionControl.fromInt(congestionControl), Priority.fromInt(priority), express),
                    attachmentBytes?.into()
                )
                reply = Reply.Success(replierId?.let { ZenohId(it) }, sample)
            } else {
                reply = Reply.Error(
                    replierId?.let { ZenohId(it) },
                    payload.into(),
                    Encoding(encodingId, schema = encodingSchema)
                )
            }
            callback.run(reply)
        }
        getViaJNI(
            jniSession.sessionPtr.get(),
            keyExpr.jniKeyExpr?.ptr ?: 0,
            keyExpr.keyExpr,
            getCallback,
            timeout.toMillis(),
            onClose::run
        )
        return receiver
    }

    fun declareToken(jniSession: JNISession, keyExpr: KeyExpr): LivelinessToken {
        val ptr = declareTokenViaJNI(jniSession.sessionPtr.get(), keyExpr.jniKeyExpr?.ptr ?: 0, keyExpr.keyExpr)
        return LivelinessToken(JNILivelinessToken(ptr))
    }

    fun declareSubscriber(
        jniSession: JNISession,
        keyExpr: KeyExpr,
        callback: Callback<Sample>,
        history: Boolean,
        onClose: () -> Unit
    ): CallbackSubscriber {
        val subCallback =
            JNISubscriberCallback { keyExpr2, payload, encodingId, encodingSchema, kind, timestampNTP64, timestampIsValid, attachmentBytes, express: Boolean, priority: Int, congestionControl: Int ->
                val timestamp = if (timestampIsValid) TimeStamp(timestampNTP64) else null
                val sample = Sample(
                    KeyExpr(keyExpr2, null),
                    payload.into(),
                    Encoding(encodingId, schema = encodingSchema),
                    SampleKind.fromInt(kind),
                    timestamp,
                    QoS(CongestionControl.fromInt(congestionControl), Priority.fromInt(priority), express),
                    attachmentBytes?.into()
                )
                callback.run(sample)
            }
        val ptr = declareSubscriberViaJNI(
            jniSession.sessionPtr.get(),
            keyExpr.jniKeyExpr?.ptr ?: 0,
            keyExpr.keyExpr,
            subCallback,
            history,
            onClose
        )
        return CallbackSubscriber(keyExpr, JNISubscriber(ptr))
    }

    fun <R> declareSubscriber(
        jniSession: JNISession,
        keyExpr: KeyExpr,
        callback: Callback<Sample>,
        receiver: R,
        history: Boolean,
        onClose: () -> Unit
    ): HandlerSubscriber<R> {
        val subCallback =
            JNISubscriberCallback { keyExpr2, payload, encodingId, encodingSchema, kind, timestampNTP64, timestampIsValid, attachmentBytes, express: Boolean, priority: Int, congestionControl: Int ->
                val timestamp = if (timestampIsValid) TimeStamp(timestampNTP64) else null
                val sample = Sample(
                    KeyExpr(keyExpr2, null),
                    payload.into(),
                    Encoding(encodingId, schema = encodingSchema),
                    SampleKind.fromInt(kind),
                    timestamp,
                    QoS(CongestionControl.fromInt(congestionControl), Priority.fromInt(priority), express),
                    attachmentBytes?.into()
                )
                callback.run(sample)
            }
        val ptr = declareSubscriberViaJNI(
            jniSession.sessionPtr.get(),
            keyExpr.jniKeyExpr?.ptr ?: 0,
            keyExpr.keyExpr,
            subCallback,
            history,
            onClose
        )
        return HandlerSubscriber(keyExpr, JNISubscriber(ptr), receiver)
    }

    private external fun getViaJNI(
        sessionPtr: Long,
        keyExprPtr: Long,
        keyExprString: String,
        callback: JNIGetCallback,
        timeoutMs: Long,
        onClose: JNIOnCloseCallback
    )

    private external fun declareTokenViaJNI(sessionPtr: Long, keyExprPtr: Long, keyExprString: String): Long

    private external fun declareSubscriberViaJNI(
        sessionPtr: Long,
        keyExprPtr: Long,
        keyExprString: String,
        callback: JNISubscriberCallback,
        history: Boolean,
        onClose: JNIOnCloseCallback
    ): Long
}
