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
import io.zenoh.handlers.BlockingQueueHandler
import io.zenoh.handlers.Callback
import io.zenoh.handlers.Handler
import io.zenoh.jni.JNILiveliness
import io.zenoh.keyexpr.KeyExpr
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
    fun declareToken(keyExpr: KeyExpr): LivelinessToken {
        val jniSession = session.jniSession ?: throw Session.sessionClosedException
        return JNILiveliness.declareToken(jniSession, keyExpr)
    }

    /**
     * Query the liveliness tokens with matching key expressions.
     *
     * @param keyExpr The [KeyExpr] for the query.
     * @param timeout Optional timeout of the query, defaults to 10 secs.
     */
    @JvmOverloads
    fun get(
        keyExpr: KeyExpr,
        timeout: Duration = Duration.ofMillis(10000),
    ): BlockingQueue<Optional<Reply>> {
        val jniSession = session.jniSession ?: throw Session.sessionClosedException
        val handler = BlockingQueueHandler<Reply>(LinkedBlockingDeque())
        return JNILiveliness.get(
            jniSession,
            keyExpr,
            handler::handle,
            receiver = handler.receiver(),
            timeout,
            onClose = handler::onClose
        )
    }

    /**
     * Query the liveliness tokens with matching key expressions.
     *
     * @param keyExpr The [KeyExpr] for the query.
     * @param callback [Callback] to handle the incoming replies.
     * @param timeout Optional timeout of the query, defaults to 10 secs.
     */
    @JvmOverloads
    fun get(
        keyExpr: KeyExpr, callback: Callback<Reply>, timeout: Duration = Duration.ofMillis(10000)
    ) {
        val jniSession = session.jniSession ?: throw Session.sessionClosedException
        return JNILiveliness.get(jniSession, keyExpr, callback, Unit, timeout, {})
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
    fun <R> get(
        keyExpr: KeyExpr, handler: Handler<Reply, R>, timeout: Duration = Duration.ofMillis(10000)
    ): R {
        val jniSession = session.jniSession ?: throw Session.sessionClosedException
        val callback = handler::handle
        return JNILiveliness.get(
            jniSession,
            keyExpr,
            callback,
            handler.receiver(),
            timeout,
            onClose = handler::onClose
        )
    }

    /**
     * Create a [Subscriber] for liveliness changes matching the given key expression.
     *
     * @param keyExpr The [KeyExpr] the subscriber will be listening to.
     * @param callback The [Callback] to be run when a liveliness change is received.
     */
    @JvmOverloads
    fun declareSubscriber(
        keyExpr: KeyExpr,
        callback: Callback<Sample>,
        options: SubscriberOptions = SubscriberOptions()
    ): Subscriber<Void> {
        val jniSession = session.jniSession ?: throw Session.sessionClosedException
        return JNILiveliness.declareSubscriber(
            jniSession,
            keyExpr,
            callback,
            null,
            options.history,
            fun() {}
        )
    }

    /**
     * Create a [Subscriber] for liveliness changes matching the given key expression.
     *
     * @param R The [Handler.receiver] type.
     * @param keyExpr The [KeyExpr] the subscriber will be listening to.
     * @param handler [Handler] to handle liveliness changes events.
     */
    @JvmOverloads
    fun <R> declareSubscriber(
        keyExpr: KeyExpr,
        handler: Handler<Sample, R>,
        options: SubscriberOptions = SubscriberOptions()
    ): Subscriber<R> {
        val jniSession = session.jniSession ?: throw Session.sessionClosedException
        return JNILiveliness.declareSubscriber(
            jniSession,
            keyExpr,
            handler::handle,
            handler.receiver(),
            options.history,
            handler::onClose
        )
    }

    @JvmOverloads
    fun declareSubscriber(
        keyExpr: KeyExpr,
        options: SubscriberOptions = SubscriberOptions()
    ): Subscriber<BlockingQueue<Optional<Sample>>> {
        val handler = BlockingQueueHandler<Sample>(LinkedBlockingDeque())
        val jniSession = session.jniSession ?: throw Session.sessionClosedException
        return JNILiveliness.declareSubscriber(
            jniSession,
            keyExpr,
            handler::handle,
            handler.receiver(),
            options.history,
            handler::onClose
        )
    }

    data class SubscriberOptions(var history: Boolean = false)
}
