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
import io.zenoh.replyCallbackOf
import io.zenoh.sampleCallbackOf
import io.zenoh.exceptions.ZError
import io.zenoh.exceptions.throwZError
import io.zenoh.handlers.BlockingQueueHandler
import io.zenoh.handlers.Callback
import io.zenoh.handlers.Handler
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.pubsub.CallbackSubscriber
import io.zenoh.pubsub.HandlerSubscriber
import io.zenoh.pubsub.Subscriber
import io.zenoh.query.Reply
import io.zenoh.sample.Sample
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
        val zSession = session.zSession ?: throw Session.sessionClosedException
        return LivelinessToken(io.zenoh.jni.session.livelinessDeclareToken(zSession, keyExpr.flat, throwZError))
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
        val zSession = session.zSession ?: throw Session.sessionClosedException
        val handler = BlockingQueueHandler<Reply>(LinkedBlockingDeque())
        io.zenoh.jni.session.livelinessGet(
            zSession,
            keyExpr.flat,
            timeout.toMillis(),
            replyCallbackOf { handler.handle(it) },
            { handler.onClose() },
            throwZError
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
        val zSession = session.zSession ?: throw Session.sessionClosedException
        io.zenoh.jni.session.livelinessGet(
            zSession,
            keyExpr.flat,
            timeout.toMillis(),
            replyCallbackOf { callback.run(it) },
            { },
            throwZError
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
        val zSession = session.zSession ?: throw Session.sessionClosedException
        io.zenoh.jni.session.livelinessGet(
            zSession,
            keyExpr.flat,
            timeout.toMillis(),
            replyCallbackOf { handler.handle(it) },
            { handler.onClose() },
            throwZError
        )
        return handler.receiver()
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
        val zSession = session.zSession ?: throw Session.sessionClosedException
        val zSubscriber = io.zenoh.jni.session.livelinessDeclareSubscriber(
            zSession,
            keyExpr.flat,
            options.history,
            sampleCallbackOf { handler.handle(it) },
            { handler.onClose() },
            throwZError
        )
        return HandlerSubscriber(keyExpr, zSubscriber, handler.receiver())
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
        val zSession = session.zSession ?: throw Session.sessionClosedException
        val zSubscriber = io.zenoh.jni.session.livelinessDeclareSubscriber(
            zSession,
            keyExpr.flat,
            options.history,
            sampleCallbackOf { callback.run(it) },
            { },
            throwZError
        )
        return CallbackSubscriber(keyExpr, zSubscriber)
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
        val zSession = session.zSession ?: throw Session.sessionClosedException
        val zSubscriber = io.zenoh.jni.session.livelinessDeclareSubscriber(
            zSession,
            keyExpr.flat,
            options.history,
            sampleCallbackOf { handler.handle(it) },
            { handler.onClose() },
            throwZError
        )
        return HandlerSubscriber(keyExpr, zSubscriber, handler.receiver())
    }
}

/**
 * Options for the [Liveliness] subscriber.
 */
data class LivelinessSubscriberOptions(var history: Boolean = false)
