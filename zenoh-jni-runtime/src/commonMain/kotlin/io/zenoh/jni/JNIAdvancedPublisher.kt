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
import io.zenoh.jni.callbacks.JNIMatchingListenerCallback
import io.zenoh.jni.callbacks.JNIOnCloseCallback

/**
 * Adapter class for a native Zenoh AdvancedPublisher.
 *
 * @property ptr Raw pointer to the underlying native AdvancedPublisher.
 */
public class JNIAdvancedPublisher(private val ptr: Long) {

    @Throws(ZError::class)
    fun put(payload: ByteArray, encodingId: Int, encodingSchema: String?, attachment: ByteArray?) {
        putViaJNI(ptr, payload, encodingId, encodingSchema, attachment)
    }

    @Throws(ZError::class)
    fun delete(attachment: ByteArray?) {
        deleteViaJNI(ptr, attachment)
    }

    @Throws(ZError::class)
    fun declareMatchingListener(callback: JNIMatchingListenerCallback, onClose: JNIOnCloseCallback): Long =
        declareMatchingListenerViaJNI(ptr, callback, onClose)

    @Throws(ZError::class)
    fun declareBackgroundMatchingListener(callback: JNIMatchingListenerCallback, onClose: JNIOnCloseCallback) =
        declareBackgroundMatchingListenerViaJNI(ptr, callback, onClose)

    @Throws(ZError::class)
    fun getMatchingStatus(): Boolean = getMatchingStatusViaJNI(ptr)

    fun close() {
        freePtrViaJNI(ptr)
    }

    @Throws(ZError::class)
    private external fun putViaJNI(
        ptr: Long, payload: ByteArray, encodingId: Int, encodingSchema: String?, attachment: ByteArray?
    )

    @Throws(ZError::class)
    private external fun deleteViaJNI(ptr: Long, attachment: ByteArray?)

    @Throws(ZError::class)
    private external fun declareMatchingListenerViaJNI(
        ptr: Long, callback: JNIMatchingListenerCallback, onClose: JNIOnCloseCallback
    ): Long

    @Throws(ZError::class)
    private external fun declareBackgroundMatchingListenerViaJNI(
        ptr: Long, callback: JNIMatchingListenerCallback, onClose: JNIOnCloseCallback
    )

    @Throws(ZError::class)
    private external fun getMatchingStatusViaJNI(ptr: Long): Boolean

    private external fun freePtrViaJNI(ptr: Long)
}
