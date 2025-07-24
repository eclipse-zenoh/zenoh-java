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

import io.zenoh.*
import io.zenoh.bytes.Encoding
import io.zenoh.exceptions.ZError
import io.zenoh.handlers.Callback
import io.zenoh.jni.callbacks.JNIOnCloseCallback
import io.zenoh.jni.callbacks.JNIGetCallback
import io.zenoh.jni.callbacks.JNIQueryableCallback
import io.zenoh.jni.callbacks.JNISubscriberCallback
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.bytes.IntoZBytes
import io.zenoh.config.ZenohId
import io.zenoh.bytes.into
import io.zenoh.Config
import io.zenoh.annotations.Unstable
import io.zenoh.config.EntityGlobalId
import io.zenoh.handlers.Handler
import io.zenoh.pubsub.*
import io.zenoh.qos.CongestionControl
import io.zenoh.qos.Priority
import io.zenoh.qos.QoS
import io.zenoh.query.*
import io.zenoh.sample.Sample
import io.zenoh.sample.SampleKind
import org.apache.commons.net.ntp.TimeStamp

/** Adapter class to handle the communication with the Zenoh JNI code for a [Session]. */
internal class JNISession(val sessionPtr: Long) {

    companion object {
        init {
            ZenohLoad
        }

        @Throws(ZError::class)
        fun open(config: Config): JNISession {
            val sessionPtr = openSessionViaJNI(config.jniConfig.ptr)
            return JNISession(sessionPtr)
        }

        @Throws(ZError::class)
        private external fun openSessionViaJNI(configPtr: Long): Long
    }

    fun close() {
        closeSessionViaJNI(sessionPtr)
    }

    @Throws(ZError::class)
    fun declarePublisher(keyExpr: KeyExpr, publisherOptions: PublisherOptions): Publisher {
        val publisherRawPtr = declarePublisherViaJNI(
            keyExpr.jniKeyExpr?.ptr ?: 0,
            keyExpr.keyExpr,
            sessionPtr,
            publisherOptions.congestionControl.value,
            publisherOptions.priority.value,
            publisherOptions.express,
            publisherOptions.reliability.ordinal
        )
        return Publisher(
            keyExpr,
            publisherOptions.congestionControl,
            publisherOptions.priority,
            publisherOptions.encoding,
            JNIPublisher(publisherRawPtr),
        )
    }

    @Throws(ZError::class)
    fun <R> declareSubscriberWithHandler(
        keyExpr: KeyExpr, handler: Handler<Sample, R>
    ): HandlerSubscriber<R> {
        val subCallback =
            JNISubscriberCallback { keyExpr1, payload, encodingId, encodingSchema, kind, timestampNTP64, timestampIsValid, attachmentBytes, express: Boolean, priority: Int, congestionControl: Int ->
                val timestamp = if (timestampIsValid) TimeStamp(timestampNTP64) else null
                val sample = Sample(
                    KeyExpr(keyExpr1, null),
                    payload.into(),
                    Encoding(encodingId, schema = encodingSchema),
                    SampleKind.fromInt(kind),
                    timestamp,
                    QoS(CongestionControl.fromInt(congestionControl), Priority.fromInt(priority), express),
                    attachmentBytes?.into()
                )
                handler.handle(sample)
            }
        val subscriberRawPtr = declareSubscriberViaJNI(
            keyExpr.jniKeyExpr?.ptr ?: 0, keyExpr.keyExpr, sessionPtr, subCallback, handler::onClose
        )
        return HandlerSubscriber(keyExpr, JNISubscriber(subscriberRawPtr), handler.receiver())
    }

    @Throws(ZError::class)
    fun declareSubscriberWithCallback(
        keyExpr: KeyExpr, callback: Callback<Sample>
    ): CallbackSubscriber {
        val subCallback =
            JNISubscriberCallback { keyExpr1, payload, encodingId, encodingSchema, kind, timestampNTP64, timestampIsValid, attachmentBytes, express: Boolean, priority: Int, congestionControl: Int ->
                val timestamp = if (timestampIsValid) TimeStamp(timestampNTP64) else null
                val sample = Sample(
                    KeyExpr(keyExpr1, null),
                    payload.into(),
                    Encoding(encodingId, schema = encodingSchema),
                    SampleKind.fromInt(kind),
                    timestamp,
                    QoS(CongestionControl.fromInt(congestionControl), Priority.fromInt(priority), express),
                    attachmentBytes?.into()
                )
                callback.run(sample)
            }
        val subscriberRawPtr = declareSubscriberViaJNI(
            keyExpr.jniKeyExpr?.ptr ?: 0,
            keyExpr.keyExpr,
            sessionPtr,
            subCallback,
            fun() {}
        )
        return CallbackSubscriber(keyExpr, JNISubscriber(subscriberRawPtr))
    }

    @Throws(ZError::class)
    fun declareQueryableWithCallback(
        keyExpr: KeyExpr, callback: Callback<Query>, config: QueryableOptions
    ): CallbackQueryable {
        val queryCallback =
            JNIQueryableCallback { keyExpr1: String, selectorParams: String, payload: ByteArray?, encodingId: Int, encodingSchema: String?, attachmentBytes: ByteArray?, queryPtr: Long ->
                val jniQuery = JNIQuery(queryPtr)
                val keyExpr2 = KeyExpr(keyExpr1, null)
                val selector = if (selectorParams.isEmpty()) {
                    Selector(keyExpr2)
                } else {
                    Selector(keyExpr2, Parameters.from(selectorParams))
                }
                val query = Query(
                    keyExpr2,
                    selector,
                    payload?.into(),
                    payload?.let { Encoding(encodingId, schema = encodingSchema) },
                    attachmentBytes?.into(),
                    jniQuery
                )
                callback.run(query)
            }
        val queryableRawPtr = declareQueryableViaJNI(
            keyExpr.jniKeyExpr?.ptr ?: 0,
            keyExpr.keyExpr,
            sessionPtr,
            queryCallback,
            fun() {},
            config.complete
        )
        return CallbackQueryable(keyExpr, JNIQueryable(queryableRawPtr))
    }

    @Throws(ZError::class)
    fun <R> declareQueryableWithHandler(
        keyExpr: KeyExpr, handler: Handler<Query, R>, config: QueryableOptions
    ): HandlerQueryable<R> {
        val queryCallback =
            JNIQueryableCallback { keyExpr1: String, selectorParams: String, payload: ByteArray?, encodingId: Int, encodingSchema: String?, attachmentBytes: ByteArray?, queryPtr: Long ->
                val jniQuery = JNIQuery(queryPtr)
                val keyExpr2 = KeyExpr(keyExpr1, null)
                val selector = if (selectorParams.isEmpty()) {
                    Selector(keyExpr2)
                } else {
                    Selector(keyExpr2, Parameters.from(selectorParams))
                }
                val query = Query(
                    keyExpr2,
                    selector,
                    payload?.into(),
                    payload?.let { Encoding(encodingId, schema = encodingSchema) },
                    attachmentBytes?.into(),
                    jniQuery
                )
                handler.handle(query)
            }
        val queryableRawPtr = declareQueryableViaJNI(
            keyExpr.jniKeyExpr?.ptr ?: 0,
            keyExpr.keyExpr,
            sessionPtr,
            queryCallback,
            handler::onClose,
            config.complete
        )
        return HandlerQueryable(keyExpr, JNIQueryable(queryableRawPtr), handler.receiver())
    }

    @OptIn(Unstable::class)
    fun declareQuerier(
        keyExpr: KeyExpr,
        options: QuerierOptions
    ): Querier {
        val querierRawPtr = declareQuerierViaJNI(
            keyExpr.jniKeyExpr?.ptr ?: 0,
            keyExpr.keyExpr,
            sessionPtr,
            options.target.ordinal,
            options.consolidationMode.ordinal,
            options.congestionControl.ordinal,
            options.priority.ordinal,
            options.express,
            options.timeout.toMillis()
        )
        return Querier(
            keyExpr,
            QoS(
                congestionControl = options.congestionControl,
                priority = options.priority,
                express = options.express
            ),
            JNIQuerier(querierRawPtr)
        )
    }

    @Throws(ZError::class)
    fun performGetWithCallback(
        intoSelector: IntoSelector,
        callback: Callback<Reply>,
        options: GetOptions
    ) {
        val getCallback = JNIGetCallback {
                replierZid: ByteArray?,
                replierEid: Int,
                success: Boolean,
                keyExpr: String?,
                payload1: ByteArray,
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
                    KeyExpr(keyExpr!!, null),
                    payload1.into(),
                    Encoding(encodingId, schema = encodingSchema),
                    SampleKind.fromInt(kind),
                    timestamp,
                    QoS(CongestionControl.fromInt(congestionControl), Priority.fromInt(priority), express),
                    attachmentBytes?.into()
                )
                reply = Reply.Success(replierZid?.let { EntityGlobalId(ZenohId(it), replierEid.toUInt()) }, sample)
            } else {
                reply = Reply.Error(
                    replierZid?.let { EntityGlobalId(ZenohId(it), replierEid.toUInt()) },
                    payload1.into(),
                    Encoding(encodingId, schema = encodingSchema)
                )
            }
            callback.run(reply)
        }

        val selector = intoSelector.into()
        getViaJNI(
            selector.keyExpr.jniKeyExpr?.ptr ?: 0,
            selector.keyExpr.keyExpr,
            selector.parameters?.toString(),
            sessionPtr,
            getCallback,
            fun() {},
            options.timeout.toMillis(),
            options.target.ordinal,
            options.consolidation.ordinal,
            options.attachment?.into()?.bytes,
            options.payload?.into()?.bytes,
            options.encoding?.id ?: Encoding.defaultEncoding().id,
            options.encoding?.schema,
            options.qos.congestionControl.value,
            options.qos.priority.value,
            options.qos.express
        )
    }

    @Throws(ZError::class)
    fun <R> performGetWithHandler(
        intoSelector: IntoSelector,
        handler: Handler<Reply, R>,
        options: GetOptions
    ): R {
        val getCallback = JNIGetCallback {
                replierZid: ByteArray?,
                replierEid: Int,
                success: Boolean,
                keyExpr: String?,
                payload1: ByteArray,
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
                    KeyExpr(keyExpr!!, null),
                    payload1.into(),
                    Encoding(encodingId, schema = encodingSchema),
                    SampleKind.fromInt(kind),
                    timestamp,
                    QoS(CongestionControl.fromInt(congestionControl), Priority.fromInt(priority), express),
                    attachmentBytes?.into()
                )
                reply = Reply.Success(replierZid?.let { EntityGlobalId(ZenohId(it), replierEid.toUInt()) }, sample)
            } else {
                reply = Reply.Error(
                    replierZid?.let { EntityGlobalId(ZenohId(it), replierEid.toUInt()) },
                    payload1.into(),
                    Encoding(encodingId, schema = encodingSchema)
                )
            }
            handler.handle(reply)
        }

        val selector = intoSelector.into()
        getViaJNI(
            selector.keyExpr.jniKeyExpr?.ptr ?: 0,
            selector.keyExpr.keyExpr,
            selector.parameters?.toString(),
            sessionPtr,
            getCallback,
            handler::onClose,
            options.timeout.toMillis(),
            options.target.ordinal,
            options.consolidation.ordinal,
            options.attachment?.into()?.bytes,
            options.payload?.into()?.bytes,
            options.encoding?.id ?: Encoding.defaultEncoding().id,
            options.encoding?.schema,
            options.qos.congestionControl.value,
            options.qos.priority.value,
            options.qos.express
        )
        return handler.receiver()
    }

    @Throws(ZError::class)
    fun declareKeyExpr(keyExpr: String): KeyExpr {
        val ptr = declareKeyExprViaJNI(sessionPtr, keyExpr)
        return KeyExpr(keyExpr, JNIKeyExpr(ptr))
    }

    @Throws(ZError::class)
    fun undeclareKeyExpr(keyExpr: KeyExpr) {
        keyExpr.jniKeyExpr?.run {
            undeclareKeyExprViaJNI(sessionPtr, this.ptr)
            keyExpr.jniKeyExpr = null
        } ?: throw ZError("Attempting to undeclare a non declared key expression.")
    }

    @Throws(ZError::class)
    fun performPut(
        keyExpr: KeyExpr,
        payload: IntoZBytes,
        options: PutOptions,
    ) {
        val encoding = options.encoding ?: Encoding.defaultEncoding()
        putViaJNI(
            keyExpr.jniKeyExpr?.ptr ?: 0,
            keyExpr.keyExpr,
            sessionPtr,
            payload.into().bytes,
            encoding.id,
            encoding.schema,
            options.congestionControl.value,
            options.priority.value,
            options.express,
            options.attachment?.into()?.bytes,
            options.reliability.ordinal
        )
    }

    @Throws(ZError::class)
    fun performDelete(
        keyExpr: KeyExpr,
        options: DeleteOptions,
    ) {
        deleteViaJNI(
            keyExpr.jniKeyExpr?.ptr ?: 0,
            keyExpr.keyExpr,
            sessionPtr,
            options.congestionControl.value,
            options.priority.value,
            options.express,
            options.attachment?.into()?.bytes,
            options.reliability.ordinal
        )
    }

    @Throws(ZError::class)
    fun zid(): ZenohId {
        return ZenohId(getZidViaJNI(sessionPtr))
    }

    @Throws(ZError::class)
    fun peersZid(): List<ZenohId> {
        return getPeersZidViaJNI(sessionPtr).map { ZenohId(it) }
    }

    @Throws(ZError::class)
    fun routersZid(): List<ZenohId> {
        return getRoutersZidViaJNI(sessionPtr).map { ZenohId(it) }
    }

    @Throws(ZError::class)
    private external fun getZidViaJNI(ptr: Long): ByteArray

    @Throws(ZError::class)
    private external fun getPeersZidViaJNI(ptr: Long): List<ByteArray>

    @Throws(ZError::class)
    private external fun getRoutersZidViaJNI(ptr: Long): List<ByteArray>

    @Throws(ZError::class)
    private external fun closeSessionViaJNI(ptr: Long)

    @Throws(ZError::class)
    private external fun declarePublisherViaJNI(
        keyExprPtr: Long,
        keyExprString: String,
        sessionPtr: Long,
        congestionControl: Int,
        priority: Int,
        express: Boolean,
        reliability: Int
    ): Long

    @Throws(ZError::class)
    private external fun declareSubscriberViaJNI(
        keyExprPtr: Long,
        keyExprString: String,
        sessionPtr: Long,
        callback: JNISubscriberCallback,
        onClose: JNIOnCloseCallback,
    ): Long

    @Throws(ZError::class)
    private external fun declareQueryableViaJNI(
        keyExprPtr: Long,
        keyExprString: String,
        sessionPtr: Long,
        callback: JNIQueryableCallback,
        onClose: JNIOnCloseCallback,
        complete: Boolean
    ): Long

    @Throws(ZError::class)
    private external fun declareQuerierViaJNI(
        keyExprPtr: Long,
        keyExprString: String,
        sessionPtr: Long,
        target: Int,
        consolidation: Int,
        congestionControl: Int,
        priority: Int,
        express: Boolean,
        timeoutMs: Long
    ): Long

    @Throws(ZError::class)
    private external fun declareKeyExprViaJNI(sessionPtr: Long, keyExpr: String): Long

    @Throws(ZError::class)
    private external fun undeclareKeyExprViaJNI(sessionPtr: Long, keyExprPtr: Long)

    @Throws(ZError::class)
    private external fun getViaJNI(
        keyExprPtr: Long,
        keyExprString: String,
        selectorParams: String?,
        sessionPtr: Long,
        callback: JNIGetCallback,
        onClose: JNIOnCloseCallback,
        timeoutMs: Long,
        target: Int,
        consolidation: Int,
        attachmentBytes: ByteArray?,
        payload: ByteArray?,
        encodingId: Int,
        encodingSchema: String?,
        congestionControl: Int,
        priority: Int,
        express: Boolean,
    )

    @Throws(ZError::class)
    private external fun putViaJNI(
        keyExprPtr: Long,
        keyExprString: String,
        sessionPtr: Long,
        valuePayload: ByteArray,
        valueEncoding: Int,
        valueEncodingSchema: String?,
        congestionControl: Int,
        priority: Int,
        express: Boolean,
        attachmentBytes: ByteArray?,
        reliability: Int
    )

    @Throws(ZError::class)
    private external fun deleteViaJNI(
        keyExprPtr: Long,
        keyExprString: String,
        sessionPtr: Long,
        congestionControl: Int,
        priority: Int,
        express: Boolean,
        attachmentBytes: ByteArray?,
        reliability: Int
    )
}
