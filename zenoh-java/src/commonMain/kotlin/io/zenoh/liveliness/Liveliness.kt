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

package io.zenoh.liveliness

import io.zenoh.Session
import io.zenoh.bytes.Encoding
import io.zenoh.bytes.into
import io.zenoh.config.EntityGlobalId
import io.zenoh.config.ZenohId
import io.zenoh.exceptions.ZError
import io.zenoh.handlers.BlockingQueueHandler
import io.zenoh.handlers.Callback
import io.zenoh.handlers.Handler
import io.zenoh.jni.callbacks.JNIGetCallback
import io.zenoh.jni.callbacks.JNISubscriberCallback
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.pubsub.CallbackSubscriber
import io.zenoh.pubsub.HandlerSubscriber
import io.zenoh.pubsub.Subscriber
import io.zenoh.qos.CongestionControl
import io.zenoh.qos.Priority
import io.zenoh.qos.QoS
import io.zenoh.query.Reply
import io.zenoh.sample.Sample
import io.zenoh.sample.SampleKind
import org.apache.commons.net.ntp.TimeStamp
import java.time.Duration
import java.util.*
import java.util.concurrent.BlockingQueue
import java.util.concurrent.LinkedBlockingDeque

/**
 * A structure with functions to declare a [LivelinessToken],
 * query existing [LivelinessToken]s and subscribe to liveliness changes.
 *
 * A [LivelinessToken] is a token which liveliness is tied
 * to the Zenoh [Session] and can be monitored by remote applications.
 *
 * The [Liveliness] instance can be obtained with the [Session.liveliness] function
 * of the [Session] instance.
 */
class Liveliness internal constructor(private val session: Session) {

    /**
     * Create a LivelinessToken for the given key expression.
     */
    @Throws(ZError::class)
    fun declareToken(keyExpr: KeyExpr): LivelinessToken {
        val jniSession = session.jniSession ?: throw Session.sessionClosedException
        return LivelinessToken(jniSession.declareLivelinessToken(keyExpr.jniKeyExpr, keyExpr.keyExpr))
    }

    /**
     * Query the liveliness tokens with matching key expressions.
     *
     * @param keyExpr The [KeyExpr] for the query.
     * @param timeout Optional timeout of the query, defaults to 10 secs.
     */
    @JvmOverloads
    @Throws(ZError::class)
    fun get(
        keyExpr: KeyExpr,
        timeout: Duration = Duration.ofMillis(10000),
    ): BlockingQueue<Optional<Reply>> {
        val jniSession = session.jniSession ?: throw Session.sessionClosedException
        val handler = BlockingQueueHandler<Reply>(LinkedBlockingDeque())
        val getCallback = buildGetCallback(handler::handle)
        jniSession.livelinessGet(
            keyExpr.jniKeyExpr,
            keyExpr.keyExpr,
            getCallback,
            timeout.toMillis(),
            handler::onClose
        )
        return handler.receiver()
    }

    /**
     * Query the liveliness tokens with matching key expressions.
     *
     * @param keyExpr The [KeyExpr] for the query.
     * @param callback [Callback] to handle the incoming replies.
     * @param timeout Optional timeout of the query, defaults to 10 secs.
     */
    @JvmOverloads
    @Throws(ZError::class)
    fun get(
        keyExpr: KeyExpr, callback: Callback<Reply>, timeout: Duration = Duration.ofMillis(10000)
    ) {
        val jniSession = session.jniSession ?: throw Session.sessionClosedException
        jniSession.livelinessGet(
            keyExpr.jniKeyExpr,
            keyExpr.keyExpr,
            buildGetCallback(callback),
            timeout.toMillis(),
            fun() {}
        )
    }

    /**
     * Query the liveliness tokens with matching key expressions.
     *
     * @param R The [Handler.receiver] type.
     * @param keyExpr The [KeyExpr] for the query.
     * @param handler [Handler] to deal with the incoming replies.
     * @param timeout Optional timeout of the query, defaults to 10 secs.
     */
    @JvmOverloads
    @Throws(ZError::class)
    fun <R> get(
        keyExpr: KeyExpr, handler: Handler<Reply, R>, timeout: Duration = Duration.ofMillis(10000)
    ): R {
        val jniSession = session.jniSession ?: throw Session.sessionClosedException
        jniSession.livelinessGet(
            keyExpr.jniKeyExpr,
            keyExpr.keyExpr,
            buildGetCallback(handler::handle),
            timeout.toMillis(),
            handler::onClose
        )
        return handler.receiver()
    }

    private fun buildGetCallback(callback: Callback<Reply>): JNIGetCallback =
        JNIGetCallback { replierZid, replierEid, success, keyExpr2, payload, encodingId, encodingSchema, kind, timestampNTP64, timestampIsValid, attachmentBytes, express, priority, congestionControl ->
            val reply: Reply = if (success) {
                val timestamp = if (timestampIsValid) TimeStamp(timestampNTP64) else null
                Reply.Success(
                    replierZid?.let { EntityGlobalId(ZenohId(it), replierEid.toUInt()) },
                    Sample(
                        KeyExpr(keyExpr2!!, null),
                        payload.into(),
                        Encoding(encodingId, schema = encodingSchema),
                        SampleKind.fromInt(kind),
                        timestamp,
                        QoS(CongestionControl.fromInt(congestionControl), Priority.fromInt(priority), express),
                        attachmentBytes?.into()
                    )
                )
            } else {
                Reply.Error(replierZid?.let { EntityGlobalId(ZenohId(it), replierEid.toUInt()) }, payload.into(), Encoding(encodingId, schema = encodingSchema))
            }
            callback.run(reply)
        }

    /**
     * Create a [Subscriber] for liveliness changes matching the given key expression.
     *
     * @param keyExpr The [KeyExpr] the subscriber will be listening to.
     * @param options Optional [LivelinessSubscriberOptions] parameter for subscriber configuration.
     */
    @JvmOverloads
    @Throws(ZError::class)
    fun declareSubscriber(
        keyExpr: KeyExpr,
        options: LivelinessSubscriberOptions = LivelinessSubscriberOptions()
    ): HandlerSubscriber<BlockingQueue<Optional<Sample>>> {
        val handler = BlockingQueueHandler<Sample>(LinkedBlockingDeque())
        val jniSession = session.jniSession ?: throw Session.sessionClosedException
        val subCallback = buildSubscriberCallback(handler::handle)
        return HandlerSubscriber(keyExpr, jniSession.declareLivelinessSubscriber(keyExpr.jniKeyExpr, keyExpr.keyExpr, subCallback, options.history, handler::onClose), handler.receiver())
    }

    /**
     * Create a [Subscriber] for liveliness changes matching the given key expression.
     *
     * @param keyExpr The [KeyExpr] the subscriber will be listening to.
     * @param callback The [Callback] to be run when a liveliness change is received.
     * @param options Optional [LivelinessSubscriberOptions] parameter for subscriber configuration.
     */
    @JvmOverloads
    @Throws(ZError::class)
    fun declareSubscriber(
        keyExpr: KeyExpr,
        callback: Callback<Sample>,
        options: LivelinessSubscriberOptions = LivelinessSubscriberOptions()
    ): CallbackSubscriber {
        val jniSession = session.jniSession ?: throw Session.sessionClosedException
        val subCallback = buildSubscriberCallback(callback)
        return CallbackSubscriber(keyExpr, jniSession.declareLivelinessSubscriber(keyExpr.jniKeyExpr, keyExpr.keyExpr, subCallback, options.history, fun() {}))
    }

    /**
     * Create a [Subscriber] for liveliness changes matching the given key expression.
     *
     * @param R The [Handler.receiver] type.
     * @param keyExpr The [KeyExpr] the subscriber will be listening to.
     * @param handler [Handler] to handle liveliness changes events.
     * @param options Optional [LivelinessSubscriberOptions] parameter for subscriber configuration.
     */
    @JvmOverloads
    @Throws(ZError::class)
    fun <R> declareSubscriber(
        keyExpr: KeyExpr,
        handler: Handler<Sample, R>,
        options: LivelinessSubscriberOptions = LivelinessSubscriberOptions()
    ): HandlerSubscriber<R> {
        val jniSession = session.jniSession ?: throw Session.sessionClosedException
        val subCallback = buildSubscriberCallback(handler::handle)
        return HandlerSubscriber(keyExpr, jniSession.declareLivelinessSubscriber(keyExpr.jniKeyExpr, keyExpr.keyExpr, subCallback, options.history, handler::onClose), handler.receiver())
    }

    private fun buildSubscriberCallback(callback: Callback<Sample>): JNISubscriberCallback =
        JNISubscriberCallback { keyExpr2, payload, encodingId, encodingSchema, kind, timestampNTP64, timestampIsValid, attachmentBytes, express, priority, congestionControl ->
            val timestamp = if (timestampIsValid) TimeStamp(timestampNTP64) else null
            callback.run(
                Sample(
                    KeyExpr(keyExpr2, null),
                    payload.into(),
                    Encoding(encodingId, schema = encodingSchema),
                    SampleKind.fromInt(kind),
                    timestamp,
                    QoS(CongestionControl.fromInt(congestionControl), Priority.fromInt(priority), express),
                    attachmentBytes?.into()
                )
            )
        }
}

/**
 * Options for the [Liveliness] subscriber.
 */
data class LivelinessSubscriberOptions(var history: Boolean = false)
