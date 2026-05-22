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
import io.zenoh.jni.ZError
import io.zenoh.jni.callbacks.JniCallback
import io.zenoh.jni.callbacks.JniZQueryCallback
import io.zenoh.jni.callbacks.JniZReplyCallback
import io.zenoh.jni.callbacks.JniSampleCallback

/**
 * Typed `NativeHandle` for a native Zenoh `Session`. The lock-and-
 * pointer machinery lives in [NativeHandle]; this class only adds the
 * `companion`-side factory and a few helpers that thread Kotlin's
 * `Long?`-encoded keyExpr handles through the auto-generated wrappers
 * in [JNIWrappers].
 */
public class JniZSession(initialPtr: Long) : NativeHandle(initialPtr) {

    companion object {
        init {
            ZenohLoad
        }

        @Throws(ZError::class)
        fun open(config: JniZConfig): JniZSession =
            JniZSession(JNIWrappers.openSession(config).peek())
    }

    @Throws(ZError::class)
    fun declarePublisher(
        keyExprHandle: NativeHandle?,
        keyExprString: String,
        congestionControl: JniCongestionControl,
        priority: JniPriority,
        express: Boolean,
        reliability: JniReliability
    ): JniZPublisher = JniZPublisher(
        JNIWrappers.declarePublisher(
            this,
            (keyExprHandle ?: keyExprString),
            congestionControl,
            priority,
            express,
            reliability,
        ).peek()
    )

    @Throws(ZError::class)
    fun declareSubscriber(
        keyExprHandle: NativeHandle?,
        keyExprString: String,
        callback: JniSampleCallback,
        onClose: JniCallback,
    ): JniZSubscriber = JniZSubscriber(
        JNIWrappers.declareSubscriber(
            this,
            (keyExprHandle ?: keyExprString),
            callback,
            onClose,
        ).peek()
    )

    @Throws(ZError::class)
    fun declareQueryable(
        keyExprHandle: NativeHandle?,
        keyExprString: String,
        callback: JniZQueryCallback,
        onClose: JniCallback,
        complete: Boolean
    ): JniZQueryable = JniZQueryable(
        JNIWrappers.declareQueryable(
            this,
            (keyExprHandle ?: keyExprString),
            callback,
            onClose,
            complete,
        ).peek()
    )

    @Throws(ZError::class)
    fun declareQuerier(
        keyExprHandle: NativeHandle?,
        keyExprString: String,
        target: JniQueryTarget,
        consolidation: JniConsolidationMode,
        congestionControl: JniCongestionControl,
        priority: JniPriority,
        express: Boolean,
        timeoutMs: Long,
        acceptReplies: JniReplyKeyExpr
    ): JniZQuerier = JniZQuerier(
        JNIWrappers.declareQuerier(
            this,
            (keyExprHandle ?: keyExprString),
            target,
            consolidation,
            congestionControl,
            priority,
            express,
            timeoutMs,
            acceptReplies,
        ).peek()
    )

    @Throws(ZError::class)
    fun declareKeyExpr(keyExpr: String): JniZKeyExpr =
        JNIWrappers.declareKeyExpr(this, keyExpr) as JniZKeyExpr

    @Throws(ZError::class)
    fun undeclareKeyExpr(keyExpr: JniZKeyExpr) =
        JNIWrappers.undeclareKeyExpr(this, keyExpr)

    @Throws(ZError::class)
    fun get(
        keyExprHandle: NativeHandle?,
        keyExprString: String,
        selectorParams: String?,
        callback: JniZReplyCallback,
        onClose: JniCallback,
        timeoutMs: Long,
        target: JniQueryTarget,
        consolidation: JniConsolidationMode,
        attachmentBytes: ByteArray?,
        payload: ByteArray?,
        encoding: JniZEncoding?,
        congestionControl: JniCongestionControl,
        priority: JniPriority,
        express: Boolean,
        acceptReplies: JniReplyKeyExpr,
    ) = JNIWrappers.get(
        this,
        (keyExprHandle ?: keyExprString),
        selectorParams,
        callback,
        onClose,
        timeoutMs,
        target,
        consolidation,
        attachmentBytes,
        payload,
        encoding,
        congestionControl,
        priority,
        express,
        acceptReplies,
    )

    @Throws(ZError::class)
    fun put(
        keyExprHandle: NativeHandle?,
        keyExprString: String,
        valuePayload: ByteArray,
        valueEncoding: JniZEncoding,
        congestionControl: JniCongestionControl,
        priority: JniPriority,
        express: Boolean,
        attachmentBytes: ByteArray?,
        reliability: JniReliability
    ) = JNIWrappers.put(
        this,
        (keyExprHandle ?: keyExprString),
        valuePayload,
        valueEncoding,
        congestionControl,
        priority,
        express,
        attachmentBytes,
        reliability,
    )

    @Throws(ZError::class)
    fun delete(
        keyExprHandle: NativeHandle?,
        keyExprString: String,
        congestionControl: JniCongestionControl,
        priority: JniPriority,
        express: Boolean,
        attachmentBytes: ByteArray?,
        reliability: JniReliability
    ) = JNIWrappers.delete(
        this,
        (keyExprHandle ?: keyExprString),
        congestionControl,
        priority,
        express,
        attachmentBytes,
        reliability,
    )

    @Throws(ZError::class)
    fun getZid(): ByteArray = JNIWrappers.getZid(this)

    @Throws(ZError::class)
    fun getPeersZid(): List<ByteArray> = JNIWrappers.getPeersZid(this)

    @Throws(ZError::class)
    fun getRoutersZid(): List<ByteArray> = JNIWrappers.getRoutersZid(this)

    @Throws(ZError::class)
    fun declareAdvancedSubscriber(
        keyExprHandle: NativeHandle?,
        keyExprStr: String,
        callback: JniSampleCallback,
        onClose: JniCallback,
        history: HistoryConfig?,
        recovery: RecoveryConfig?,
        subscriberDetection: Boolean,
    ): JniZAdvancedSubscriber = JniZAdvancedSubscriber(
        JNIWrappers.declareAdvancedSubscriber(
            this,
            (keyExprHandle ?: keyExprStr),
            callback,
            onClose,
            history,
            recovery,
            subscriberDetection,
        ).peek()
    )

    @Throws(ZError::class)
    fun declareAdvancedPublisher(
        keyExprHandle: NativeHandle?,
        keyExprStr: String,
        congestionControl: JniCongestionControl,
        priority: JniPriority,
        isExpress: Boolean,
        reliability: JniReliability,
        cache: CacheConfig?,
        sampleMissDetection: MissDetectionConfig?,
        publisherDetection: Boolean,
    ): JniZAdvancedPublisher = JniZAdvancedPublisher(
        JNIWrappers.declareAdvancedPublisher(
            this,
            (keyExprHandle ?: keyExprStr),
            congestionControl,
            priority,
            isExpress,
            reliability,
            cache,
            sampleMissDetection,
            publisherDetection,
        ).peek()
    )

    @Throws(ZError::class)
    fun declareLivelinessToken(
        keyExprHandle: NativeHandle?,
        keyExprString: String,
    ): JniZLivelinessToken = JniZLivelinessToken(
        JNIWrappers.declareLivelinessToken(this, (keyExprHandle ?: keyExprString)).peek()
    )

    @Throws(ZError::class)
    fun declareLivelinessSubscriber(
        keyExprHandle: NativeHandle?,
        keyExprString: String,
        callback: JniSampleCallback,
        history: Boolean,
        onClose: JniCallback,
    ): JniZSubscriber = JniZSubscriber(
        JNIWrappers.declareLivelinessSubscriber(
            this,
            (keyExprHandle ?: keyExprString),
            callback,
            onClose,
            history,
        ).peek()
    )

    @Throws(ZError::class)
    fun livelinessGet(
        keyExprHandle: NativeHandle?,
        keyExprString: String,
        callback: JniZReplyCallback,
        timeoutMs: Long,
        onClose: JniCallback,
    ) = JNIWrappers.livelinessGet(
        this,
        (keyExprHandle ?: keyExprString),
        callback,
        onClose,
        timeoutMs,
    )

    /**
     * Consume this session via the generator-emitted `dropSession`.
     * `Session::drop` runs `self.close().wait()` internally so this is
     * the complete teardown.
     */
    fun close() = JNIWrappers.dropSession(this)
}
