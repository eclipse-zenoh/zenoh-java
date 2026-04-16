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

import io.zenoh.ZenohLoad
import io.zenoh.exceptions.ZError

/** Adapter for native Zenoh key expressions. Factory methods return raw primitives. */
public class JNIKeyExpr(public val ptr: Long) {

    companion object {
        init {
            ZenohLoad
        }

        @Throws(ZError::class)
        internal external fun tryFromViaJNI(keyExpr: String): String

        @Throws(ZError::class)
        internal external fun autocanonizeViaJNI(keyExpr: String): String

        @Throws(ZError::class)
        internal external fun intersectsViaJNI(ptrA: Long, keyExprA: String, ptrB: Long, keyExprB: String): Boolean

        @Throws(ZError::class)
        internal external fun includesViaJNI(ptrA: Long, keyExprA: String, ptrB: Long, keyExprB: String): Boolean

        /** Returns SetIntersectionLevel ordinal as Int. Callers convert to SetIntersectionLevel. */
        @Throws(ZError::class)
        internal external fun relationToViaJNI(ptrA: Long, keyExprA: String, ptrB: Long, keyExprB: String): Int

        @Throws(ZError::class)
        internal external fun joinViaJNI(ptrA: Long, keyExprA: String, other: String): String

        @Throws(ZError::class)
        internal external fun concatViaJNI(ptrA: Long, keyExprA: String, other: String): String
    }

    fun close() {
        freePtrViaJNI(ptr)
    }

    private external fun freePtrViaJNI(ptr: Long)
}
