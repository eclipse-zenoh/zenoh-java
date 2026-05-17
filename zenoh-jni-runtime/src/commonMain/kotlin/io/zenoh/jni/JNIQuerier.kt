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

/**
 * Typed [NativeHandle] for a native Zenoh `Querier`. `get` and
 * `freePtrViaJNI` are hand-written JNI entry points (not in
 * `zenoh-flat`), so they stay as `external fun` methods routed through
 * the inherited [withPtr].
 */
public class JNIQuerier(initialPtr: Long) : NativeHandle(initialPtr) {

    @Throws(ZError::class)
    fun get(
        keyExprHandle: NativeHandle?,
        keyExprString: String,
        parameters: String?,
        callback: JNIGetCallback,
        onClose: JNIOnCloseCallback,
        attachmentBytes: ByteArray?,
        payload: ByteArray?,
        encoding: JNIEncoding?,
    ) = withPtr { ptr ->
        if (keyExprHandle != null) {
            keyExprHandle.withPtr { kePtr ->
                getViaJNI(ptr, kePtr, parameters, callback, onClose, attachmentBytes, payload, encoding)
            }
        } else {
            getViaJNI(ptr, keyExprString, parameters, callback, onClose, attachmentBytes, payload, encoding)
        }
    }

    @Throws(ZError::class)
    private external fun getViaJNI(
        querierPtr: Long,
        keyExpr: Any,
        parameters: String?,
        callback: JNIGetCallback,
        onClose: JNIOnCloseCallback,
        attachmentBytes: ByteArray?,
        payload: ByteArray?,
        encoding: JNIEncoding?,
    )

    private external fun freePtrViaJNI(ptr: Long)

    fun close() = close { freePtrViaJNI(it) }
}
