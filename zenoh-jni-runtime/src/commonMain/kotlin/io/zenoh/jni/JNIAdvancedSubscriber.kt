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

import io.zenoh.exceptions.ZError
import io.zenoh.jni.callbacks.JNIOnCloseCallback
import io.zenoh.jni.callbacks.JNISampleMissListenerCallback
import io.zenoh.jni.callbacks.JNISubscriberCallback

/**
 * Adapter class for a native Zenoh AdvancedSubscriber.
 *
 * @property ptr Raw pointer to the underlying native AdvancedSubscriber.
 */
public class JNIAdvancedSubscriber(private val ptr: Long) {

    @Throws(ZError::class)
    fun declareDetectPublishersSubscriber(
        history: Boolean,
        callback: JNISubscriberCallback,
        onClose: JNIOnCloseCallback,
    ): Long = declareDetectPublishersSubscriberViaJNI(ptr, history, callback, onClose)

    @Throws(ZError::class)
    fun declareBackgroundDetectPublishersSubscriber(
        history: Boolean,
        callback: JNISubscriberCallback,
        onClose: JNIOnCloseCallback,
    ) = declareBackgroundDetectPublishersSubscriberViaJNI(ptr, history, callback, onClose)

    @Throws(ZError::class)
    fun declareSampleMissListener(
        callback: JNISampleMissListenerCallback,
        onClose: JNIOnCloseCallback,
    ): Long = declareSampleMissListenerViaJNI(ptr, callback, onClose)

    @Throws(ZError::class)
    fun declareBackgroundSampleMissListener(
        callback: JNISampleMissListenerCallback,
        onClose: JNIOnCloseCallback,
    ) = declareBackgroundSampleMissListenerViaJNI(ptr, callback, onClose)

    fun close() {
        freePtrViaJNI(ptr)
    }

    @Throws(ZError::class)
    private external fun declareDetectPublishersSubscriberViaJNI(
        ptr: Long, history: Boolean, callback: JNISubscriberCallback, onClose: JNIOnCloseCallback
    ): Long

    @Throws(ZError::class)
    private external fun declareBackgroundDetectPublishersSubscriberViaJNI(
        ptr: Long, history: Boolean, callback: JNISubscriberCallback, onClose: JNIOnCloseCallback
    )

    @Throws(ZError::class)
    private external fun declareSampleMissListenerViaJNI(
        ptr: Long, callback: JNISampleMissListenerCallback, onClose: JNIOnCloseCallback
    ): Long

    @Throws(ZError::class)
    private external fun declareBackgroundSampleMissListenerViaJNI(
        ptr: Long, callback: JNISampleMissListenerCallback, onClose: JNIOnCloseCallback
    )

    private external fun freePtrViaJNI(ptr: Long)
}
