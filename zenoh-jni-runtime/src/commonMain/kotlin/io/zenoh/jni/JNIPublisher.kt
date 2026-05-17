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
 * Typed [NativeHandle] for a native Zenoh `Publisher`. `put` and
 * `delete` delegate to the generator-emitted wrappers in
 * [JNIWrappers]; `freePtrViaJNI` is hand-written in Rust (the
 * by-value `drop_publisher` shape isn't a `#[prebindgen]` fn because
 * `Publisher` is not `Clone`), so [close] keeps the direct call.
 */
public class JNIPublisher(initialPtr: Long) : NativeHandle(initialPtr) {

    @Throws(ZError::class)
    fun put(payload: ByteArray, encoding: JNIEncoding, attachment: ByteArray?) =
        JNIWrappers.putPublisher(this, payload, encoding, attachment)

    @Throws(ZError::class)
    fun delete(attachment: ByteArray?) =
        JNIWrappers.deletePublisher(this, attachment)

    fun close() = close { freePtrViaJNI(it) }

    private external fun freePtrViaJNI(ptr: Long)
}
