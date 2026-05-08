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
import io.zenoh.jni.JNINative.closeSessionViaJNI
import io.zenoh.jni.JNINative.declareAdvancedPublisherViaJNI
import io.zenoh.jni.JNINative.declareAdvancedSubscriberViaJNI
import io.zenoh.jni.JNINative.declareKeyExprViaJNI
import io.zenoh.jni.JNINative.declarePublisherViaJNI
import io.zenoh.jni.JNINative.declareQuerierViaJNI
import io.zenoh.jni.JNINative.declareQueryableViaJNI
import io.zenoh.jni.JNINative.declareSubscriberViaJNI
import io.zenoh.jni.JNINative.deleteViaJNI
import io.zenoh.jni.JNINative.dropSessionViaJNI
import io.zenoh.jni.JNINative.getPeersZidViaJNI
import io.zenoh.jni.JNINative.getRoutersZidViaJNI
import io.zenoh.jni.JNINative.getViaJNI
import io.zenoh.jni.JNINative.getZidViaJNI
import io.zenoh.jni.JNINative.openSessionViaJNI
import io.zenoh.jni.JNINative.putViaJNI
import io.zenoh.jni.JNINative.undeclareKeyExprViaJNI
import io.zenoh.jni.callbacks.JNIGetCallback
import io.zenoh.jni.callbacks.JNIOnCloseCallback
import io.zenoh.jni.callbacks.JNIQueryableCallback
import io.zenoh.jni.callbacks.JNISubscriberCallback

/** Adapter class to handle communication with the Zenoh JNI code for a Session. */
public class JNISession(internal val sessionPtr: Long) {

    companion object {
        init {
            ZenohLoad
        }

        @Throws(ZError::class)
        fun open(config: JNIConfig): JNISession {
            val sessionPtr = openSessionViaJNI(config.ptr)
            return JNISession(sessionPtr)
        }
    }

    @Throws(ZError::class)
    fun declarePublisher(
        jniKeyExpr: JNIKeyExpr?,
        keyExprString: String,
        congestionControl: Int,
        priority: Int,
        express: Boolean,
        reliability: Int
    ): JNIPublisher = JNIPublisher(declarePublisherViaJNI(sessionPtr, JNIKeyExpr.of(jniKeyExpr, keyExprString), congestionControl, priority, express, reliability))

    @Throws(ZError::class)
    fun declareSubscriber(
        jniKeyExpr: JNIKeyExpr?,
        keyExprString: String,
        callback: JNISubscriberCallback,
        onClose: JNIOnCloseCallback,
    ): JNISubscriber = JNISubscriber(declareSubscriberViaJNI(sessionPtr, JNIKeyExpr.of(jniKeyExpr, keyExprString), callback, onClose))

    @Throws(ZError::class)
    fun declareQueryable(
        jniKeyExpr: JNIKeyExpr?,
        keyExprString: String,
        callback: JNIQueryableCallback,
        onClose: JNIOnCloseCallback,
        complete: Boolean
    ): JNIQueryable = JNIQueryable(declareQueryableViaJNI(sessionPtr, JNIKeyExpr.of(jniKeyExpr, keyExprString), callback, onClose, complete))

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
    ): JNIQuerier = JNIQuerier(declareQuerierViaJNI(sessionPtr, JNIKeyExpr.of(jniKeyExpr, keyExprString), target, consolidation, congestionControl, priority, express, timeoutMs, acceptReplies))

    @Throws(ZError::class)
    fun declareKeyExpr(keyExpr: String): JNIKeyExpr = declareKeyExprViaJNI(sessionPtr, keyExpr)

    @Throws(ZError::class)
    fun undeclareKeyExpr(jniKeyExpr: JNIKeyExpr) = undeclareKeyExprViaJNI(sessionPtr, jniKeyExpr)

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
        encoding: JNIEncoding?,
        congestionControl: Int,
        priority: Int,
        express: Boolean,
        acceptReplies: Int,
    ) = getViaJNI(sessionPtr, JNIKeyExpr.of(jniKeyExpr, keyExprString), selectorParams, callback, onClose, timeoutMs, target, consolidation, attachmentBytes, payload, encoding, congestionControl, priority, express, acceptReplies)

    @Throws(ZError::class)
    fun put(
        jniKeyExpr: JNIKeyExpr?,
        keyExprString: String,
        valuePayload: ByteArray,
        valueEncoding: JNIEncoding,
        congestionControl: Int,
        priority: Int,
        express: Boolean,
        attachmentBytes: ByteArray?,
        reliability: Int
    ) = putViaJNI(sessionPtr, JNIKeyExpr.of(jniKeyExpr, keyExprString), valuePayload, valueEncoding, congestionControl, priority, express, attachmentBytes, reliability)

    @Throws(ZError::class)
    fun delete(
        jniKeyExpr: JNIKeyExpr?,
        keyExprString: String,
        congestionControl: Int,
        priority: Int,
        express: Boolean,
        attachmentBytes: ByteArray?,
        reliability: Int
    ) = deleteViaJNI(sessionPtr, JNIKeyExpr.of(jniKeyExpr, keyExprString), congestionControl, priority, express, attachmentBytes, reliability)

    @Throws(ZError::class)
    fun getZid(): ByteArray = getZidViaJNI(sessionPtr)

    @Throws(ZError::class)
    fun getPeersZid(): List<ByteArray> = getPeersZidViaJNI(sessionPtr)

    @Throws(ZError::class)
    fun getRoutersZid(): List<ByteArray> = getRoutersZidViaJNI(sessionPtr)

    @Throws(ZError::class)
    fun declareAdvancedSubscriber(
        jniKeyExpr: JNIKeyExpr?,
        keyExprStr: String,
        callback: JNISubscriberCallback,
        onClose: JNIOnCloseCallback,
        history: HistoryConfig?,
        recovery: RecoveryConfig?,
        subscriberDetection: Boolean,
    ): JNIAdvancedSubscriber = JNIAdvancedSubscriber(
        declareAdvancedSubscriberViaJNI(
            sessionPtr,
            JNIKeyExpr.of(jniKeyExpr, keyExprStr),
            callback,
            onClose,
            history,
            recovery,
            subscriberDetection,
        )
    )

    @Throws(ZError::class)
    fun declareAdvancedPublisher(
        jniKeyExpr: JNIKeyExpr?,
        keyExprStr: String,
        congestionControl: Int,
        priority: Int,
        isExpress: Boolean,
        reliability: Int,
        cache: CacheConfig?,
        sampleMissDetection: MissDetectionConfig?,
        publisherDetection: Boolean,
    ): JNIAdvancedPublisher = JNIAdvancedPublisher(
        declareAdvancedPublisherViaJNI(
            sessionPtr,
            JNIKeyExpr.of(jniKeyExpr, keyExprStr),
            congestionControl,
            priority,
            isExpress,
            reliability,
            cache,
            sampleMissDetection,
            publisherDetection,
        )
    )

    @Throws(ZError::class)
    fun declareLivelinessToken(jniKeyExpr: JNIKeyExpr?, keyExprString: String): JNILivelinessToken =
        JNILivelinessToken(declareLivelinessTokenViaJNI(sessionPtr, JNIKeyExpr.of(jniKeyExpr, keyExprString)))

    @Throws(ZError::class)
    private external fun declareLivelinessTokenViaJNI(sessionPtr: Long, keyExpr: JNIKeyExpr): Long

    @Throws(ZError::class)
    fun declareLivelinessSubscriber(
        jniKeyExpr: JNIKeyExpr?,
        keyExprString: String,
        callback: JNISubscriberCallback,
        history: Boolean,
        onClose: JNIOnCloseCallback,
    ): JNISubscriber = JNISubscriber(declareLivelinessSubscriberViaJNI(sessionPtr, JNIKeyExpr.of(jniKeyExpr, keyExprString), callback, history, onClose))

    @Throws(ZError::class)
    private external fun declareLivelinessSubscriberViaJNI(
        sessionPtr: Long,
        keyExpr: JNIKeyExpr,
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
    ) = livelinessGetViaJNI(sessionPtr, JNIKeyExpr.of(jniKeyExpr, keyExprString), callback, timeoutMs, onClose)

    @Throws(ZError::class)
    private external fun livelinessGetViaJNI(
        sessionPtr: Long,
        keyExpr: JNIKeyExpr,
        callback: JNIGetCallback,
        timeoutMs: Long,
        onClose: JNIOnCloseCallback,
    )

    fun close() {
        closeSessionViaJNI(sessionPtr)
        dropSessionViaJNI(sessionPtr)
    }
}
