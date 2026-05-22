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
import io.zenoh.jni.callbacks.JniZMatchingStatusCallback

/** Typed [NativeHandle] for a native Zenoh `AdvancedPublisher`. */
public class JniZAdvancedPublisher(initialPtr: Long) : NativeHandle(initialPtr) {

    @Throws(ZError::class)
    fun put(payload: ByteArray, encoding: JniZEncoding, attachment: ByteArray?) = withPtr { ptr ->
        putViaJNI(ptr, payload, encoding, attachment)
    }

    @Throws(ZError::class)
    fun delete(attachment: ByteArray?) = withPtr { ptr ->
        deleteViaJNI(ptr, attachment)
    }

    @Throws(ZError::class)
    fun declareMatchingListener(callback: JniZMatchingStatusCallback, onClose: JniCallback): JniZMatchingListener =
        withPtr { ptr -> JniZMatchingListener(declareMatchingListenerViaJNI(ptr, callback, onClose)) }

    @Throws(ZError::class)
    fun declareBackgroundMatchingListener(callback: JniZMatchingStatusCallback, onClose: JniCallback) =
        withPtr { ptr -> declareBackgroundMatchingListenerViaJNI(ptr, callback, onClose) }

    @Throws(ZError::class)
    fun getMatchingStatus(): Boolean = withPtr { ptr -> getMatchingStatusViaJNI(ptr) }

    fun free() = free { freePtrViaJNI(it) }

    @Throws(ZError::class)
    private external fun putViaJNI(
        ptr: Long, payload: ByteArray, encoding: JniZEncoding, attachment: ByteArray?
    )

    @Throws(ZError::class)
    private external fun deleteViaJNI(ptr: Long, attachment: ByteArray?)

    @Throws(ZError::class)
    private external fun declareMatchingListenerViaJNI(
        ptr: Long, callback: JniZMatchingStatusCallback, onClose: JniCallback
    ): Long

    @Throws(ZError::class)
    private external fun declareBackgroundMatchingListenerViaJNI(
        ptr: Long, callback: JniZMatchingStatusCallback, onClose: JniCallback
    )

    @Throws(ZError::class)
    private external fun getMatchingStatusViaJNI(ptr: Long): Boolean

    private external fun freePtrViaJNI(ptr: Long)
}
