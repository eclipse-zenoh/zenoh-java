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
import io.zenoh.jni.callbacks.JNIGetCallback
import io.zenoh.jni.callbacks.JNIOnCloseCallback
import io.zenoh.jni.callbacks.JNIQueryableCallback
import io.zenoh.jni.callbacks.JNISampleCallback

/**
 * Typed `NativeHandle` for a native Zenoh `Session`. The lock-and-
 * pointer machinery lives in [NativeHandle]; this class only adds the
 * `companion`-side factory and a few helpers that thread Kotlin's
 * `Long?`-encoded keyExpr handles through the auto-generated wrappers
 * in [JNIWrappers].
 */
public class JNISession(initialPtr: Long) : NativeHandle(initialPtr) {

    companion object {
        init {
            ZenohLoad
        }

        @Throws(ZError::class)
        fun open(config: JNIConfig): JNISession =
            JNISession(JNIWrappers.openSession(config).peek())
    }

    @Throws(ZError::class)
    fun declarePublisher(
        keyExprHandle: NativeHandle?,
        keyExprString: String,
        congestionControl: Int,
        priority: Int,
        express: Boolean,
        reliability: Int
    ): JNIPublisher = JNIPublisher(
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
        callback: JNISampleCallback,
        onClose: JNIOnCloseCallback,
    ): JNISubscriber = JNISubscriber(
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
        callback: JNIQueryableCallback,
        onClose: JNIOnCloseCallback,
        complete: Boolean
    ): JNIQueryable = JNIQueryable(
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
        target: Int,
        consolidation: Int,
        congestionControl: Int,
        priority: Int,
        express: Boolean,
        timeoutMs: Long,
        acceptReplies: Int
    ): JNIQuerier = JNIQuerier(
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
    fun declareKeyExpr(keyExpr: String): JNIKeyExpr =
        JNIWrappers.declareKeyExpr(this, keyExpr) as JNIKeyExpr

    @Throws(ZError::class)
    fun undeclareKeyExpr(keyExpr: JNIKeyExpr) =
        JNIWrappers.undeclareKeyExpr(this, keyExpr)

    @Throws(ZError::class)
    fun get(
        keyExprHandle: NativeHandle?,
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
        valueEncoding: JNIEncoding,
        congestionControl: Int,
        priority: Int,
        express: Boolean,
        attachmentBytes: ByteArray?,
        reliability: Int
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
        congestionControl: Int,
        priority: Int,
        express: Boolean,
        attachmentBytes: ByteArray?,
        reliability: Int
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
        callback: JNISampleCallback,
        onClose: JNIOnCloseCallback,
        history: HistoryConfig?,
        recovery: RecoveryConfig?,
        subscriberDetection: Boolean,
    ): JNIAdvancedSubscriber = JNIAdvancedSubscriber(
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
        congestionControl: Int,
        priority: Int,
        isExpress: Boolean,
        reliability: Int,
        cache: CacheConfig?,
        sampleMissDetection: MissDetectionConfig?,
        publisherDetection: Boolean,
    ): JNIAdvancedPublisher = JNIAdvancedPublisher(
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
    ): JNILivelinessToken = JNILivelinessToken(
        JNIWrappers.declareLivelinessToken(this, (keyExprHandle ?: keyExprString)).peek()
    )

    @Throws(ZError::class)
    fun declareLivelinessSubscriber(
        keyExprHandle: NativeHandle?,
        keyExprString: String,
        callback: JNISampleCallback,
        history: Boolean,
        onClose: JNIOnCloseCallback,
    ): JNISubscriber = JNISubscriber(
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
        callback: JNIGetCallback,
        timeoutMs: Long,
        onClose: JNIOnCloseCallback,
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
