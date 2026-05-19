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

/** Typed [NativeHandle] for a native Zenoh `Query`. The reply methods
 *  are hand-written JNI entry points (not in `zenoh-flat`). */
public class JNIQuery(initialPtr: Long) : NativeHandle(initialPtr) {

    @Throws(ZError::class)
    fun replySuccess(
        keyExprHandle: NativeHandle?,
        keyExprString: String,
        payload: ByteArray,
        encoding: JNIEncoding,
        timestampEnabled: Boolean,
        timestampNtp64: Long,
        attachment: ByteArray?,
        qosExpress: Boolean,
    ) = withPtr { ptr ->
        if (keyExprHandle != null) {
            keyExprHandle.withPtr { kePtr ->
                replySuccessViaJNI(ptr, kePtr, payload, encoding, timestampEnabled, timestampNtp64, attachment, qosExpress)
            }
        } else {
            replySuccessViaJNI(ptr, keyExprString, payload, encoding, timestampEnabled, timestampNtp64, attachment, qosExpress)
        }
    }

    @Throws(ZError::class)
    fun replyError(errorPayload: ByteArray, encoding: JNIEncoding) = withPtr { ptr ->
        replyErrorViaJNI(ptr, errorPayload, encoding)
    }

    @Throws(ZError::class)
    fun replyDelete(
        keyExprHandle: NativeHandle?,
        keyExprString: String,
        timestampEnabled: Boolean,
        timestampNtp64: Long,
        attachment: ByteArray?,
        qosExpress: Boolean,
    ) = withPtr { ptr ->
        if (keyExprHandle != null) {
            keyExprHandle.withPtr { kePtr ->
                replyDeleteViaJNI(ptr, kePtr, timestampEnabled, timestampNtp64, attachment, qosExpress)
            }
        } else {
            replyDeleteViaJNI(ptr, keyExprString, timestampEnabled, timestampNtp64, attachment, qosExpress)
        }
    }

    fun free() = free { freePtrViaJNI(it) }

    @Throws(ZError::class)
    private external fun replySuccessViaJNI(
        queryPtr: Long,
        keyExpr: Any,
        valuePayload: ByteArray,
        valueEncoding: JNIEncoding,
        timestampEnabled: Boolean,
        timestampNtp64: Long,
        attachment: ByteArray?,
        qosExpress: Boolean,
    )

    @Throws(ZError::class)
    private external fun replyErrorViaJNI(
        queryPtr: Long,
        errorValuePayload: ByteArray,
        errorValueEncoding: JNIEncoding,
    )

    @Throws(ZError::class)
    private external fun replyDeleteViaJNI(
        queryPtr: Long,
        keyExpr: Any,
        timestampEnabled: Boolean,
        timestampNtp64: Long,
        attachment: ByteArray?,
        qosExpress: Boolean,
    )

    private external fun freePtrViaJNI(ptr: Long)
}
