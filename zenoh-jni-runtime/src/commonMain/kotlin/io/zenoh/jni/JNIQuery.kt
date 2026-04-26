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
        encoding: JNIEncoding,
        timestampEnabled: Boolean,
        timestampNtp64: Long,
        attachment: ByteArray?,
        qosExpress: Boolean,
    ) {
        replySuccessViaJNI(ptr, JNIKeyExpr.of(jniKeyExpr, keyExprString), payload, encoding, timestampEnabled, timestampNtp64, attachment, qosExpress)
    }

    @Throws(ZError::class)
    fun replyError(errorPayload: ByteArray, encoding: JNIEncoding) {
        replyErrorViaJNI(ptr, errorPayload, encoding)
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
        replyDeleteViaJNI(ptr, JNIKeyExpr.of(jniKeyExpr, keyExprString), timestampEnabled, timestampNtp64, attachment, qosExpress)
    }

    fun close() {
        freePtrViaJNI(ptr)
    }

    @Throws(ZError::class)
    private external fun replySuccessViaJNI(
        queryPtr: Long,
        keyExpr: JNIKeyExpr,
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
        keyExpr: JNIKeyExpr,
        timestampEnabled: Boolean,
        timestampNtp64: Long,
        attachment: ByteArray?,
        qosExpress: Boolean,
    )

    private external fun freePtrViaJNI(ptr: Long)
}
