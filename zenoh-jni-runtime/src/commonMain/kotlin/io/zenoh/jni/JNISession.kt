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

package io.zenoh.jni

import io.zenoh.ZenohLoad
import io.zenoh.exceptions.ZError
import io.zenoh.jni.callbacks.JNIGetCallback
import io.zenoh.jni.callbacks.JNIOnCloseCallback
import io.zenoh.jni.callbacks.JNIQueryableCallback
import io.zenoh.jni.callbacks.JNISubscriberCallback

/** Adapter class to handle communication with the Zenoh JNI code for a Session. */
public class JNISession(public val sessionPtr: Long) {

    companion object {
        init {
            ZenohLoad
        }

        @Throws(ZError::class)
        fun open(config: JNIConfig): JNISession {
            val sessionPtr = openSessionViaJNI(config.ptr)
            return JNISession(sessionPtr)
        }

        @JvmStatic
        @Throws(ZError::class)
        private external fun openSessionViaJNI(configPtr: Long): Long
    }

    @Throws(ZError::class)
    private external fun closeSessionViaJNI(ptr: Long)

    @Throws(ZError::class)
    fun declarePublisher(
        jniKeyExpr: JNIKeyExpr?,
        keyExprString: String,
        congestionControl: Int,
        priority: Int,
        express: Boolean,
        reliability: Int
    ): JNIPublisher = JNIPublisher(declarePublisherViaJNI(sessionPtr, jniKeyExpr?.ptr ?: 0, keyExprString, congestionControl, priority, express, reliability))

    @Throws(ZError::class)
    private external fun declarePublisherViaJNI(
        sessionPtr: Long,
        keyExprPtr: Long,
        keyExprString: String,
        congestionControl: Int,
        priority: Int,
        express: Boolean,
        reliability: Int
    ): Long

    @Throws(ZError::class)
    fun declareSubscriber(
        jniKeyExpr: JNIKeyExpr?,
        keyExprString: String,
        callback: JNISubscriberCallback,
        onClose: JNIOnCloseCallback,
    ): JNISubscriber = JNISubscriber(declareSubscriberViaJNI(sessionPtr, jniKeyExpr?.ptr ?: 0, keyExprString, callback, onClose))

    @Throws(ZError::class)
    private external fun declareSubscriberViaJNI(
        sessionPtr: Long,
        keyExprPtr: Long,
        keyExprString: String,
        callback: JNISubscriberCallback,
        onClose: JNIOnCloseCallback,
    ): Long

    @Throws(ZError::class)
    fun declareQueryable(
        jniKeyExpr: JNIKeyExpr?,
        keyExprString: String,
        callback: JNIQueryableCallback,
        onClose: JNIOnCloseCallback,
        complete: Boolean
    ): JNIQueryable = JNIQueryable(declareQueryableViaJNI(sessionPtr, jniKeyExpr?.ptr ?: 0, keyExprString, callback, onClose, complete))

    @Throws(ZError::class)
    private external fun declareQueryableViaJNI(
        sessionPtr: Long,
        keyExprPtr: Long,
        keyExprString: String,
        callback: JNIQueryableCallback,
        onClose: JNIOnCloseCallback,
        complete: Boolean
    ): Long

    @Throws(ZError::class)
    fun declareQuerier(
        jniKeyExpr: JNIKeyExpr?,
        keyExprString: String,
        target: Int,
        consolidation: Int,
        congestionControl: Int,
        priority: Int,
        express: Boolean,
        timeoutMs: Long,
        acceptReplies: Int
    ): JNIQuerier = JNIQuerier(declareQuerierViaJNI(sessionPtr, jniKeyExpr?.ptr ?: 0, keyExprString, target, consolidation, congestionControl, priority, express, timeoutMs, acceptReplies))

    @Throws(ZError::class)
    private external fun declareQuerierViaJNI(
        sessionPtr: Long,
        keyExprPtr: Long,
        keyExprString: String,
        target: Int,
        consolidation: Int,
        congestionControl: Int,
        priority: Int,
        express: Boolean,
        timeoutMs: Long,
        acceptReplies: Int
    ): Long

    @Throws(ZError::class)
    fun declareKeyExpr(keyExpr: String): JNIKeyExpr = JNIKeyExpr(declareKeyExprViaJNI(sessionPtr, keyExpr))

    @Throws(ZError::class)
    private external fun declareKeyExprViaJNI(sessionPtr: Long, keyExpr: String): Long

    @Throws(ZError::class)
    fun undeclareKeyExpr(jniKeyExpr: JNIKeyExpr) = undeclareKeyExprViaJNI(sessionPtr, jniKeyExpr.ptr)

    @Throws(ZError::class)
    private external fun undeclareKeyExprViaJNI(sessionPtr: Long, keyExprPtr: Long)

    @Throws(ZError::class)
    fun get(
        jniKeyExpr: JNIKeyExpr?,
        keyExprString: String,
        selectorParams: String?,
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
        acceptReplies: Int,
    ) = getViaJNI(sessionPtr, jniKeyExpr?.ptr ?: 0, keyExprString, selectorParams, callback, onClose, timeoutMs, target, consolidation, attachmentBytes, payload, encodingId, encodingSchema, congestionControl, priority, express, acceptReplies)

    @Throws(ZError::class)
    private external fun getViaJNI(
        sessionPtr: Long,
        keyExprPtr: Long,
        keyExprString: String,
        selectorParams: String?,
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
        acceptReplies: Int,
    )

    @Throws(ZError::class)
    fun put(
        jniKeyExpr: JNIKeyExpr?,
        keyExprString: String,
        valuePayload: ByteArray,
        valueEncoding: Int,
        valueEncodingSchema: String?,
        congestionControl: Int,
        priority: Int,
        express: Boolean,
        attachmentBytes: ByteArray?,
        reliability: Int
    ) = putViaJNI(sessionPtr, jniKeyExpr?.ptr ?: 0, keyExprString, valuePayload, valueEncoding, valueEncodingSchema, congestionControl, priority, express, attachmentBytes, reliability)

    @Throws(ZError::class)
    private external fun putViaJNI(
        sessionPtr: Long,
        keyExprPtr: Long,
        keyExprString: String,
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
    fun delete(
        jniKeyExpr: JNIKeyExpr?,
        keyExprString: String,
        congestionControl: Int,
        priority: Int,
        express: Boolean,
        attachmentBytes: ByteArray?,
        reliability: Int
    ) = deleteViaJNI(sessionPtr, jniKeyExpr?.ptr ?: 0, keyExprString, congestionControl, priority, express, attachmentBytes, reliability)

    @Throws(ZError::class)
    private external fun deleteViaJNI(
        sessionPtr: Long,
        keyExprPtr: Long,
        keyExprString: String,
        congestionControl: Int,
        priority: Int,
        express: Boolean,
        attachmentBytes: ByteArray?,
        reliability: Int
    )

    @Throws(ZError::class)
    fun getZid(): ByteArray = getZidViaJNI(sessionPtr)

    @Throws(ZError::class)
    private external fun getZidViaJNI(ptr: Long): ByteArray

    @Throws(ZError::class)
    fun getPeersZid(): List<ByteArray> = getPeersZidViaJNI(sessionPtr)

    @Throws(ZError::class)
    private external fun getPeersZidViaJNI(ptr: Long): List<ByteArray>

    @Throws(ZError::class)
    fun getRoutersZid(): List<ByteArray> = getRoutersZidViaJNI(sessionPtr)

    @Throws(ZError::class)
    private external fun getRoutersZidViaJNI(ptr: Long): List<ByteArray>

    @Throws(ZError::class)
    fun declareAdvancedSubscriber(
        jniKeyExpr: JNIKeyExpr?,
        keyExprStr: String,
        historyConfigEnabled: Boolean,
        historyDetectLatePublishers: Boolean,
        historyMaxSamples: Long,
        historyMaxAgeSeconds: Double,
        recoveryConfigEnabled: Boolean,
        recoveryConfigIsHeartbeat: Boolean,
        recoveryQueryPeriodMs: Long,
        subscriberDetection: Boolean,
        callback: JNISubscriberCallback,
        onClose: JNIOnCloseCallback,
    ): JNIAdvancedSubscriber = JNIAdvancedSubscriber(declareAdvancedSubscriberViaJNI(sessionPtr, jniKeyExpr?.ptr ?: 0, keyExprStr, historyConfigEnabled, historyDetectLatePublishers, historyMaxSamples, historyMaxAgeSeconds, recoveryConfigEnabled, recoveryConfigIsHeartbeat, recoveryQueryPeriodMs, subscriberDetection, callback, onClose))

    @Throws(ZError::class)
    private external fun declareAdvancedSubscriberViaJNI(
        sessionPtr: Long,
        keyExprPtr: Long,
        keyExprStr: String,
        historyConfigEnabled: Boolean,
        historyDetectLatePublishers: Boolean,
        historyMaxSamples: Long,
        historyMaxAgeSeconds: Double,
        recoveryConfigEnabled: Boolean,
        recoveryConfigIsHeartbeat: Boolean,
        recoveryQueryPeriodMs: Long,
        subscriberDetection: Boolean,
        callback: JNISubscriberCallback,
        onClose: JNIOnCloseCallback,
    ): Long

    @Throws(ZError::class)
    fun declareAdvancedPublisher(
        jniKeyExpr: JNIKeyExpr?,
        keyExprStr: String,
        congestionControl: Int,
        priority: Int,
        isExpress: Boolean,
        reliability: Int,
        cacheEnabled: Boolean,
        cacheMaxSamples: Long,
        cacheRepliesPriority: Int,
        cacheRepliesCongestionControl: Int,
        cacheRepliesIsExpress: Boolean,
        sampleMissDetectionEnabled: Boolean,
        sampleMissDetectionEnableHeartbeat: Boolean,
        sampleMissDetectionHeartbeatMs: Long,
        sampleMissDetectionHeartbeatIsSporadic: Boolean,
        publisherDetection: Boolean,
    ): JNIAdvancedPublisher = JNIAdvancedPublisher(declareAdvancedPublisherViaJNI(sessionPtr, jniKeyExpr?.ptr ?: 0, keyExprStr, congestionControl, priority, isExpress, reliability, cacheEnabled, cacheMaxSamples, cacheRepliesPriority, cacheRepliesCongestionControl, cacheRepliesIsExpress, sampleMissDetectionEnabled, sampleMissDetectionEnableHeartbeat, sampleMissDetectionHeartbeatMs, sampleMissDetectionHeartbeatIsSporadic, publisherDetection))

    @Throws(ZError::class)
    private external fun declareAdvancedPublisherViaJNI(
        sessionPtr: Long,
        keyExprPtr: Long,
        keyExprStr: String,
        congestionControl: Int,
        priority: Int,
        isExpress: Boolean,
        reliability: Int,
        cacheEnabled: Boolean,
        cacheMaxSamples: Long,
        cacheRepliesPriority: Int,
        cacheRepliesCongestionControl: Int,
        cacheRepliesIsExpress: Boolean,
        sampleMissDetectionEnabled: Boolean,
        sampleMissDetectionEnableHeartbeat: Boolean,
        sampleMissDetectionHeartbeatMs: Long,
        sampleMissDetectionHeartbeatIsSporadic: Boolean,
        publisherDetection: Boolean,
    ): Long

    @Throws(ZError::class)
    fun declareLivelinessToken(jniKeyExpr: JNIKeyExpr?, keyExprString: String): JNILivelinessToken =
        JNILivelinessToken(declareLivelinessTokenViaJNI(sessionPtr, jniKeyExpr?.ptr ?: 0, keyExprString))

    @Throws(ZError::class)
    private external fun declareLivelinessTokenViaJNI(sessionPtr: Long, keyExprPtr: Long, keyExprString: String): Long

    @Throws(ZError::class)
    fun declareLivelinessSubscriber(
        jniKeyExpr: JNIKeyExpr?,
        keyExprString: String,
        callback: JNISubscriberCallback,
        history: Boolean,
        onClose: JNIOnCloseCallback,
    ): JNISubscriber = JNISubscriber(declareLivelinessSubscriberViaJNI(sessionPtr, jniKeyExpr?.ptr ?: 0, keyExprString, callback, history, onClose))

    @Throws(ZError::class)
    private external fun declareLivelinessSubscriberViaJNI(
        sessionPtr: Long,
        keyExprPtr: Long,
        keyExprString: String,
        callback: JNISubscriberCallback,
        history: Boolean,
        onClose: JNIOnCloseCallback,
    ): Long

    @Throws(ZError::class)
    fun livelinessGet(
        jniKeyExpr: JNIKeyExpr?,
        keyExprString: String,
        callback: JNIGetCallback,
        timeoutMs: Long,
        onClose: JNIOnCloseCallback,
    ) = livelinessGetViaJNI(sessionPtr, jniKeyExpr?.ptr ?: 0, keyExprString, callback, timeoutMs, onClose)

    @Throws(ZError::class)
    private external fun livelinessGetViaJNI(
        sessionPtr: Long,
        keyExprPtr: Long,
        keyExprString: String,
        callback: JNIGetCallback,
        timeoutMs: Long,
        onClose: JNIOnCloseCallback,
    )

    fun close() {
        closeSessionViaJNI(sessionPtr)
    }
}
