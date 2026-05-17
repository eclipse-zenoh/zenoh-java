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
import io.zenoh.jni.JNINative.deletePublisherViaJNI
import io.zenoh.jni.JNINative.putPublisherViaJNI

/**
 * Adapter class for a native Zenoh publisher. Uses primitive types for put/delete.
 *
 * @param initialPtr Raw pointer to the underlying native Publisher.
 */
public class JNIPublisher(initialPtr: Long) {
    private val handle = NativeHandle(initialPtr)

    @Throws(ZError::class)
    fun put(payload: ByteArray, encoding: JNIEncoding, attachment: ByteArray?) = handle.withPtr { ptr ->
        putPublisherViaJNI(ptr, payload, encoding, attachment)
    }

    @Throws(ZError::class)
    fun delete(attachment: ByteArray?) = handle.withPtr { ptr ->
        deletePublisherViaJNI(ptr, attachment)
    }

    fun close() = handle.close(::freePtrViaJNI)

    // freePtrViaJNI is hand-written in zenoh-jni/src/publisher.rs because
    // the auto-generated `opaque_arc_input` convention clones the outer
    // Arc on borrow and drops the clone on exit, so the master Arc is
    // released via this dedicated entry point.
    private external fun freePtrViaJNI(ptr: Long)
}
