//
// Copyright (c) 2023 ZettaScale Technology
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
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.keyexpr.SetIntersectionLevel

internal class JNIKeyExpr(internal val ptr: Long) {

    companion object {
        init {
            ZenohLoad
        }

        @Throws(ZError::class)
        fun tryFrom(keyExpr: String): KeyExpr {
            return KeyExpr(tryFromViaJNI(keyExpr))
        }

        @Throws(ZError::class)
        fun autocanonize(keyExpr: String): KeyExpr {
            return KeyExpr(autocanonizeViaJNI(keyExpr))
        }

        @Throws(ZError::class)
        fun intersects(keyExprA: KeyExpr, keyExprB: KeyExpr): Boolean = intersectsViaJNI(
            keyExprA.jniKeyExpr?.ptr ?: 0,
            keyExprA.keyExpr,
            keyExprB.jniKeyExpr?.ptr ?: 0,
            keyExprB.keyExpr
        )

        @Throws(ZError::class)
        fun includes(keyExprA: KeyExpr, keyExprB: KeyExpr): Boolean = includesViaJNI(
            keyExprA.jniKeyExpr?.ptr ?: 0,
            keyExprA.keyExpr,
            keyExprB.jniKeyExpr?.ptr ?: 0,
            keyExprB.keyExpr
        )

        @Throws(ZError::class)
        fun relationTo(keyExpr: KeyExpr, other: KeyExpr): SetIntersectionLevel {
            val intersection = relationToViaJNI(
                keyExpr.jniKeyExpr?.ptr ?: 0,
                keyExpr.keyExpr,
                other.jniKeyExpr?.ptr ?: 0,
                other.keyExpr
            )
            return SetIntersectionLevel.fromInt(intersection)
        }

        @Throws(ZError::class)
        fun joinViaJNI(keyExpr: KeyExpr, other: String): KeyExpr {
            return KeyExpr(joinViaJNI(keyExpr.jniKeyExpr?.ptr ?: 0, keyExpr.keyExpr, other))
        }

        @Throws(ZError::class)
        fun concatViaJNI(keyExpr: KeyExpr, other: String): KeyExpr {
            return KeyExpr(concatViaJNI(keyExpr.jniKeyExpr?.ptr ?: 0, keyExpr.keyExpr, other))
        }

        @Throws(ZError::class)
        private external fun tryFromViaJNI(keyExpr: String): String

        @Throws(ZError::class)
        private external fun autocanonizeViaJNI(keyExpr: String): String

        @Throws(ZError::class)
        private external fun intersectsViaJNI(ptrA: Long, keyExprA: String, ptrB: Long, keyExprB: String): Boolean

        @Throws(ZError::class)
        private external fun includesViaJNI(ptrA: Long, keyExprA: String, ptrB: Long, keyExprB: String): Boolean

        @Throws(ZError::class)
        private external fun relationToViaJNI(ptrA: Long, keyExprA: String, ptrB: Long, keyExprB: String): Int

        @Throws(ZError::class)
        private external fun joinViaJNI(ptrA: Long, keyExprA: String, other: String): String

        @Throws(ZError::class)
        private external fun concatViaJNI(ptrA: Long, keyExprA: String, other: String): String
    }

    fun close() {
        freePtrViaJNI(ptr)
    }

    /** Frees the underlying native KeyExpr. */
    private external fun freePtrViaJNI(ptr: Long)
}
