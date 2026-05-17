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
        keyExprHandle: Long?,
        keyExprString: String,
        congestionControl: Int,
        priority: Int,
        express: Boolean,
        reliability: Int
    ): JNIPublisher = JNIPublisher(
        JNIWrappers.declarePublisher(
            this,
            keyExprArg(keyExprHandle, keyExprString),
            congestionControl,
            priority,
            express,
            reliability,
        ).peek()
    )

    @Throws(ZError::class)
    fun declareSubscriber(
        keyExprHandle: Long?,
        keyExprString: String,
        callback: JNISampleCallback,
        onClose: JNIOnCloseCallback,
    ): JNISubscriber = JNISubscriber(
        JNIWrappers.declareSubscriber(
            this,
            keyExprArg(keyExprHandle, keyExprString),
            callback,
            onClose,
        ).peek()
    )

    @Throws(ZError::class)
    fun declareQueryable(
        keyExprHandle: Long?,
        keyExprString: String,
        callback: JNIQueryableCallback,
        onClose: JNIOnCloseCallback,
        complete: Boolean
    ): JNIQueryable = JNIQueryable(
        JNIWrappers.declareQueryable(
            this,
            keyExprArg(keyExprHandle, keyExprString),
            callback,
            onClose,
            complete,
        ).peek()
    )

    @Throws(ZError::class)
    fun declareQuerier(
        keyExprHandle: Long?,
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
            keyExprArg(keyExprHandle, keyExprString),
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
    fun declareKeyExpr(keyExpr: String): Long =
        JNIWrappers.declareKeyExpr(this, keyExpr).peek()

    @Throws(ZError::class)
    fun undeclareKeyExpr(keyExprHandle: Long) =
        JNIWrappers.undeclareKeyExpr(this, NativeHandle(keyExprHandle))

    @Throws(ZError::class)
    fun get(
        keyExprHandle: Long?,
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
        keyExprArg(keyExprHandle, keyExprString),
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
        keyExprHandle: Long?,
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
        keyExprArg(keyExprHandle, keyExprString),
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
        keyExprHandle: Long?,
        keyExprString: String,
        congestionControl: Int,
        priority: Int,
        express: Boolean,
        attachmentBytes: ByteArray?,
        reliability: Int
    ) = JNIWrappers.delete(
        this,
        keyExprArg(keyExprHandle, keyExprString),
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
        keyExprHandle: Long?,
        keyExprStr: String,
        callback: JNISampleCallback,
        onClose: JNIOnCloseCallback,
        history: HistoryConfig?,
        recovery: RecoveryConfig?,
        subscriberDetection: Boolean,
    ): JNIAdvancedSubscriber = JNIAdvancedSubscriber(
        JNIWrappers.declareAdvancedSubscriber(
            this,
            keyExprArg(keyExprHandle, keyExprStr),
            callback,
            onClose,
            history,
            recovery,
            subscriberDetection,
        ).peek()
    )

    @Throws(ZError::class)
    fun declareAdvancedPublisher(
        keyExprHandle: Long?,
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
            keyExprArg(keyExprHandle, keyExprStr),
            congestionControl,
            priority,
            isExpress,
            reliability,
            cache,
            sampleMissDetection,
            publisherDetection,
        ).peek()
    )

    // Liveliness operations don't live in zenoh-flat as `#[prebindgen]`
    // functions, so the generator doesn't emit wrappers for them. The
    // `external fun`s + their hand-written withPtr stay here.

    @Throws(ZError::class)
    fun declareLivelinessToken(keyExprHandle: Long?, keyExprString: String): JNILivelinessToken =
        withPtr { ptr ->
            JNILivelinessToken(declareLivelinessTokenViaJNI(ptr, keyExprArg(keyExprHandle, keyExprString)))
        }

    @Throws(ZError::class)
    private external fun declareLivelinessTokenViaJNI(sessionPtr: Long, keyExpr: Any): Long

    @Throws(ZError::class)
    fun declareLivelinessSubscriber(
        keyExprHandle: Long?,
        keyExprString: String,
        callback: JNISampleCallback,
        history: Boolean,
        onClose: JNIOnCloseCallback,
    ): JNISubscriber = withPtr { ptr ->
        JNISubscriber(declareLivelinessSubscriberViaJNI(ptr, keyExprArg(keyExprHandle, keyExprString), callback, history, onClose))
    }

    @Throws(ZError::class)
    private external fun declareLivelinessSubscriberViaJNI(
        sessionPtr: Long,
        keyExpr: Any,
        callback: JNISampleCallback,
        history: Boolean,
        onClose: JNIOnCloseCallback,
    ): Long

    @Throws(ZError::class)
    fun livelinessGet(
        keyExprHandle: Long?,
        keyExprString: String,
        callback: JNIGetCallback,
        timeoutMs: Long,
        onClose: JNIOnCloseCallback,
    ) = withPtr { ptr ->
        livelinessGetViaJNI(ptr, keyExprArg(keyExprHandle, keyExprString), callback, timeoutMs, onClose)
    }

    @Throws(ZError::class)
    private external fun livelinessGetViaJNI(
        sessionPtr: Long,
        keyExpr: Any,
        callback: JNIGetCallback,
        timeoutMs: Long,
        onClose: JNIOnCloseCallback,
    )

    /**
     * Consume this session via the generator-emitted `dropSession`.
     * `Session::drop` runs `self.close().wait()` internally so this is
     * the complete teardown.
     */
    fun close() = JNIWrappers.dropSession(this)
}
