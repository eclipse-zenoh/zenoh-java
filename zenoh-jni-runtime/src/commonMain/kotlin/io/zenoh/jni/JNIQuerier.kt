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
import io.zenoh.jni.callbacks.JNIGetCallback
import io.zenoh.jni.callbacks.JNIOnCloseCallback

/** Adapter class for a native Zenoh querier. */
public class JNIQuerier(private val ptr: Long) {

    @Throws(ZError::class)
    fun get(
        jniKeyExpr: JNIKeyExpr?,
        keyExprString: String,
        parameters: String?,
        callback: JNIGetCallback,
        onClose: JNIOnCloseCallback,
        attachmentBytes: ByteArray?,
        payload: ByteArray?,
        encodingId: Int,
        encodingSchema: String?,
    ) {
        getViaJNI(ptr, jniKeyExpr?.ptr ?: 0, keyExprString, parameters, callback, onClose, attachmentBytes, payload, encodingId, encodingSchema)
    }

    @Throws(ZError::class)
    private external fun getViaJNI(
        querierPtr: Long,
        keyExprPtr: Long,
        keyExprString: String,
        parameters: String?,
        callback: JNIGetCallback,
        onClose: JNIOnCloseCallback,
        attachmentBytes: ByteArray?,
        payload: ByteArray?,
        encodingId: Int,
        encodingSchema: String?,
    )

    private external fun freePtrViaJNI(ptr: Long)

    fun close() {
        freePtrViaJNI(ptr)
    }
}
