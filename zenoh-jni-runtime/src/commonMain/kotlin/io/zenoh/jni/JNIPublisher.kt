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
 * Adapter class for a native Zenoh publisher. Uses primitive types for put/delete.
 *
 * @property ptr Raw pointer to the underlying native Publisher.
 */
public class JNIPublisher(private val ptr: Long) {

    @Throws(ZError::class)
    fun put(payload: ByteArray, encodingId: Int, encodingSchema: String?, attachment: ByteArray?) {
        putViaJNI(ptr, payload, encodingId, encodingSchema, attachment)
    }

    @Throws(ZError::class)
    fun delete(attachment: ByteArray?) {
        deleteViaJNI(ptr, attachment)
    }

    fun close() {
        freePtrViaJNI(ptr)
    }

    @Throws(ZError::class)
    private external fun putViaJNI(
        ptr: Long, valuePayload: ByteArray, encodingId: Int, encodingSchema: String?, attachment: ByteArray?
    )

    @Throws(ZError::class)
    private external fun deleteViaJNI(ptr: Long, attachment: ByteArray?)

    private external fun freePtrViaJNI(ptr: Long)
}
