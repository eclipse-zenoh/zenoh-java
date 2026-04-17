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

/**
 * Adapter class for interacting with a native Zenoh Query using JNI.
 *
 * @property ptr The raw pointer to the underlying native query.
 */
public class JNIQuery(private val ptr: Long) {

    @Throws(ZError::class)
    fun replySuccess(
        jniKeyExpr: JNIKeyExpr?,
        keyExprString: String,
        payload: ByteArray,
        encodingId: Int,
        encodingSchema: String?,
        timestampEnabled: Boolean,
        timestampNtp64: Long,
        attachment: ByteArray?,
        qosExpress: Boolean,
    ) {
        replySuccessViaJNI(ptr, jniKeyExpr?.ptr ?: 0, keyExprString, payload, encodingId, encodingSchema, timestampEnabled, timestampNtp64, attachment, qosExpress)
    }

    @Throws(ZError::class)
    fun replyError(errorPayload: ByteArray, encodingId: Int, encodingSchema: String?) {
        replyErrorViaJNI(ptr, errorPayload, encodingId, encodingSchema)
    }

    @Throws(ZError::class)
    fun replyDelete(
        jniKeyExpr: JNIKeyExpr?,
        keyExprString: String,
        timestampEnabled: Boolean,
        timestampNtp64: Long,
        attachment: ByteArray?,
        qosExpress: Boolean,
    ) {
        replyDeleteViaJNI(ptr, jniKeyExpr?.ptr ?: 0, keyExprString, timestampEnabled, timestampNtp64, attachment, qosExpress)
    }

    fun close() {
        freePtrViaJNI(ptr)
    }

    @Throws(ZError::class)
    private external fun replySuccessViaJNI(
        queryPtr: Long,
        keyExprPtr: Long,
        keyExprString: String,
        valuePayload: ByteArray,
        valueEncodingId: Int,
        valueEncodingSchema: String?,
        timestampEnabled: Boolean,
        timestampNtp64: Long,
        attachment: ByteArray?,
        qosExpress: Boolean,
    )

    @Throws(ZError::class)
    private external fun replyErrorViaJNI(
        queryPtr: Long,
        errorValuePayload: ByteArray,
        errorValueEncoding: Int,
        encodingSchema: String?,
    )

    @Throws(ZError::class)
    private external fun replyDeleteViaJNI(
        queryPtr: Long,
        keyExprPtr: Long,
        keyExprString: String,
        timestampEnabled: Boolean,
        timestampNtp64: Long,
        attachment: ByteArray?,
        qosExpress: Boolean,
    )

    private external fun freePtrViaJNI(ptr: Long)
}
