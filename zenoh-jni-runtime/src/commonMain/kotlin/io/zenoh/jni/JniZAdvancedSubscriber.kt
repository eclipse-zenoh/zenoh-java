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

import io.zenoh.jni.ZError
import io.zenoh.jni.callbacks.JniCallback
import io.zenoh.jni.callbacks.JniSampleCallback
import io.zenoh.jni.callbacks.JniZSampleMissedCallback

/** Typed [NativeHandle] for a native Zenoh `AdvancedSubscriber`. */
public class JniZAdvancedSubscriber(initialPtr: Long) : NativeHandle(initialPtr) {

    @Throws(ZError::class)
    fun declareDetectPublishersSubscriber(
        history: Boolean,
        callback: JniSampleCallback,
        onClose: JniCallback,
    ): JniZSubscriber = withPtr { ptr ->
        JniZSubscriber(declareDetectPublishersSubscriberViaJNI(ptr, history, callback, onClose))
    }

    @Throws(ZError::class)
    fun declareBackgroundDetectPublishersSubscriber(
        history: Boolean,
        callback: JniSampleCallback,
        onClose: JniCallback,
    ) = withPtr { ptr ->
        declareBackgroundDetectPublishersSubscriberViaJNI(ptr, history, callback, onClose)
    }

    @Throws(ZError::class)
    fun declareSampleMissListener(
        callback: JniZSampleMissedCallback,
        onClose: JniCallback,
    ): JniZSampleMissListener = withPtr { ptr ->
        JniZSampleMissListener(declareSampleMissListenerViaJNI(ptr, callback, onClose))
    }

    @Throws(ZError::class)
    fun declareBackgroundSampleMissListener(
        callback: JniZSampleMissedCallback,
        onClose: JniCallback,
    ) = withPtr { ptr ->
        declareBackgroundSampleMissListenerViaJNI(ptr, callback, onClose)
    }

    fun free() = free { freePtrViaJNI(it) }

    @Throws(ZError::class)
    private external fun declareDetectPublishersSubscriberViaJNI(
        ptr: Long, history: Boolean, callback: JniSampleCallback, onClose: JniCallback
    ): Long

    @Throws(ZError::class)
    private external fun declareBackgroundDetectPublishersSubscriberViaJNI(
        ptr: Long, history: Boolean, callback: JniSampleCallback, onClose: JniCallback
    )

    @Throws(ZError::class)
    private external fun declareSampleMissListenerViaJNI(
        ptr: Long, callback: JniZSampleMissedCallback, onClose: JniCallback
    ): Long

    @Throws(ZError::class)
    private external fun declareBackgroundSampleMissListenerViaJNI(
        ptr: Long, callback: JniZSampleMissedCallback, onClose: JniCallback
    )

    private external fun freePtrViaJNI(ptr: Long)
}
