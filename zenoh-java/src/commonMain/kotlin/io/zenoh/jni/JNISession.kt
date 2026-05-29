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

import io.zenoh.Config
import io.zenoh.Session
import io.zenoh.ZenohLoad
import io.zenoh.annotations.Unstable
import io.zenoh.bytes.Encoding
import io.zenoh.bytes.IntoZBytes
import io.zenoh.bytes.into
import io.zenoh.config.ZenohId
import io.zenoh.exceptions.ZError
import io.zenoh.exceptions.wrapJNIExceptionAsZError
import io.zenoh.handlers.Callback
import io.zenoh.handlers.Handler
import io.zenoh.jni.session.ZSession
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.pubsub.*
import io.zenoh.qos.QoS
import io.zenoh.query.*
import io.zenoh.sample.Sample

/** Adapter class to handle the communication with the Zenoh JNI code for a [Session]. */
internal class JNISession(internal val zSession: ZSession) {

    companion object {
        init {
            ZenohLoad
        }

        @Throws(ZError::class)
        fun open(config: Config): JNISession = wrapJNIExceptionAsZError {
            JNISession(ZSession.zOpen(config.zConfig))
        }
    }

    fun close() {
        zSession.close()
    }

    @Throws(ZError::class)
    fun declarePublisher(keyExpr: KeyExpr, publisherOptions: PublisherOptions): Publisher =
        wrapJNIExceptionAsZError {
            val zPublisher = zSession.zSessionDeclarePublisher(
                keyExpr.flat,
                publisherOptions.congestionControl.jni,
                publisherOptions.priority.jni,
                publisherOptions.express,
                publisherOptions.reliability.jni
            )
            Publisher(
                keyExpr,
                publisherOptions.congestionControl,
                publisherOptions.priority,
                publisherOptions.encoding,
                zPublisher,
            )
        }

    @Throws(ZError::class)
    fun <R> declareSubscriberWithHandler(
        keyExpr: KeyExpr, handler: Handler<Sample, R>
    ): HandlerSubscriber<R> = wrapJNIExceptionAsZError {
        val zSubscriber = zSession.sessionDeclareSubscriber(
            keyExpr.flat,
            io.zenoh.jni.callbacks.SampleCallback { flat -> handler.handle(Sample.from(flat)) },
            io.zenoh.jni.callbacks.Callback { handler.onClose() }
        )
        HandlerSubscriber(keyExpr, zSubscriber, handler.receiver())
    }

    @Throws(ZError::class)
    fun declareSubscriberWithCallback(
        keyExpr: KeyExpr, callback: Callback<Sample>
    ): CallbackSubscriber = wrapJNIExceptionAsZError {
        val zSubscriber = zSession.sessionDeclareSubscriber(
            keyExpr.flat,
            io.zenoh.jni.callbacks.SampleCallback { flat -> callback.run(Sample.from(flat)) },
            io.zenoh.jni.callbacks.Callback { }
        )
        CallbackSubscriber(keyExpr, zSubscriber)
    }

    @Throws(ZError::class)
    fun declareQueryableWithCallback(
        keyExpr: KeyExpr, callback: Callback<Query>, config: QueryableOptions
    ): CallbackQueryable = wrapJNIExceptionAsZError {
        val zQueryable = zSession.sessionDeclareQueryable(
            keyExpr.flat,
            config.complete,
            io.zenoh.jni.callbacks.QueryCallback { flat -> callback.run(Query.from(flat)) },
            io.zenoh.jni.callbacks.Callback { }
        )
        CallbackQueryable(keyExpr, zQueryable)
    }

    @Throws(ZError::class)
    fun <R> declareQueryableWithHandler(
        keyExpr: KeyExpr, handler: Handler<Query, R>, config: QueryableOptions
    ): HandlerQueryable<R> = wrapJNIExceptionAsZError {
        val zQueryable = zSession.sessionDeclareQueryable(
            keyExpr.flat,
            config.complete,
            io.zenoh.jni.callbacks.QueryCallback { flat -> handler.handle(Query.from(flat)) },
            io.zenoh.jni.callbacks.Callback { handler.onClose() }
        )
        HandlerQueryable(keyExpr, zQueryable, handler.receiver())
    }

    @OptIn(Unstable::class)
    @Throws(ZError::class)
    fun declareQuerier(
        keyExpr: KeyExpr,
        options: QuerierOptions
    ): Querier = wrapJNIExceptionAsZError {
        val zQuerier = zSession.zSessionDeclareQuerier(
            keyExpr.flat,
            options.target.toFlat(),
            options.consolidationMode.toFlat(),
            options.congestionControl.jni,
            options.priority.jni,
            options.express,
            options.timeout.toMillis(),
            options.acceptReplies.toFlat()
        )
        Querier(
            keyExpr,
            QoS(
                congestionControl = options.congestionControl,
                priority = options.priority,
                express = options.express
            ),
            zQuerier
        )
    }

    @Throws(ZError::class)
    fun performGetWithCallback(
        intoSelector: IntoSelector,
        callback: Callback<Reply>,
        options: GetOptions
    ) = wrapJNIExceptionAsZError {
        val selector = intoSelector.into()
        zSession.sessionGet(
            selector.keyExpr.flat,
            selector.parameters?.toString(),
            options.timeout.toMillis(),
            options.target.toFlat(),
            options.consolidation.toFlat(),
            options.acceptReplies.toFlat(),
            options.qos.congestionControl.jni,
            options.qos.priority.jni,
            options.qos.express,
            options.payload?.into()?.inner,
            (options.encoding ?: Encoding.defaultEncoding()).toFlat(),
            options.attachment?.into()?.inner,
            io.zenoh.jni.callbacks.ReplyCallback { flat -> callback.run(Reply.from(flat)) },
            io.zenoh.jni.callbacks.Callback { }
        )
    }

    @Throws(ZError::class)
    fun <R> performGetWithHandler(
        intoSelector: IntoSelector,
        handler: Handler<Reply, R>,
        options: GetOptions
    ): R = wrapJNIExceptionAsZError {
        val selector = intoSelector.into()
        zSession.sessionGet(
            selector.keyExpr.flat,
            selector.parameters?.toString(),
            options.timeout.toMillis(),
            options.target.toFlat(),
            options.consolidation.toFlat(),
            options.acceptReplies.toFlat(),
            options.qos.congestionControl.jni,
            options.qos.priority.jni,
            options.qos.express,
            options.payload?.into()?.inner,
            (options.encoding ?: Encoding.defaultEncoding()).toFlat(),
            options.attachment?.into()?.inner,
            io.zenoh.jni.callbacks.ReplyCallback { flat -> handler.handle(Reply.from(flat)) },
            io.zenoh.jni.callbacks.Callback { handler.onClose() }
        )
        handler.receiver()
    }

    @Throws(ZError::class)
    fun declareKeyExpr(keyExpr: String): KeyExpr = wrapJNIExceptionAsZError {
        val zKeyExpr = zSession.zSessionDeclareKeyexpr(keyExpr)
        KeyExpr(io.zenoh.jni.keyexpr.KeyExpr(keyExpr, zKeyExpr))
    }

    @Throws(ZError::class)
    fun undeclareKeyExpr(keyExpr: KeyExpr) = wrapJNIExceptionAsZError {
        val handle = keyExpr.flat.keyExprNative
        if (handle == null || handle.isClosed()) {
            throw ZError("Attempting to undeclare a non declared key expression.")
        }
        zSession.zSessionUndeclareKeyexpr(handle)
    }

    @Throws(ZError::class)
    fun performPut(
        keyExpr: KeyExpr,
        payload: IntoZBytes,
        options: PutOptions,
    ) = wrapJNIExceptionAsZError {
        val encoding = options.encoding ?: Encoding.defaultEncoding()
        zSession.zSessionPut(
            keyExpr.flat,
            payload.into().inner,
            encoding.toFlat(),
            options.congestionControl.jni,
            options.priority.jni,
            options.express,
            options.attachment?.into()?.inner,
            options.reliability.jni
        )
    }

    @Throws(ZError::class)
    fun performDelete(
        keyExpr: KeyExpr,
        options: DeleteOptions,
    ) = wrapJNIExceptionAsZError {
        zSession.zSessionDelete(
            keyExpr.flat,
            options.congestionControl.jni,
            options.priority.jni,
            options.express,
            options.attachment?.into()?.inner,
            options.reliability.jni
        )
    }

    @Throws(ZError::class)
    fun zid(): ZenohId = wrapJNIExceptionAsZError {
        ZenohId(zSession.zSessionZid())
    }

    @Throws(ZError::class)
    fun peersZid(): List<ZenohId> = wrapJNIExceptionAsZError {
        zSession.zSessionPeersZid().map { ZenohId(it) }
    }

    @Throws(ZError::class)
    fun routersZid(): List<ZenohId> = wrapJNIExceptionAsZError {
        zSession.zSessionRoutersZid().map { ZenohId(it) }
    }
}
