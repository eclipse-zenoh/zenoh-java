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

/**
 * Adapter for native Zenoh key expressions.
 *
 * Carries both the native-handle pointer and the source string so it can be
 * passed across JNI as a single `JObject`. `ptr == 0L` means "not declared on
 * a session — use [str] at the native side"; `ptr != 0L` means "declared —
 * use the pointer and ignore [str]".
 */
public class JNIKeyExpr(internal val ptr: Long, internal val str: String) {

    companion object {
        init {
            ZenohLoad
        }

        /**
         * Build a JNI holder for an undeclared key expression (string-only).
         * Used at JNI call boundaries where the caller may not have a
         * declared [JNIKeyExpr] and needs to pass the raw string instead.
         */
        fun undeclared(keyExpr: String): JNIKeyExpr = JNIKeyExpr(0L, keyExpr)

        /**
         * Build a JNI holder from an optional declared expression and a
         * source string. If [declared] is non-null its pointer is preserved
         * (and the string is passed along for diagnostics / fallback);
         * otherwise the result represents the undeclared [keyExpr].
         */
        fun of(declared: JNIKeyExpr?, keyExpr: String): JNIKeyExpr =
            declared ?: JNIKeyExpr(0L, keyExpr)

        @Throws(ZError::class)
        fun tryFrom(keyExpr: String): String = JNIKeyExprNative.tryFromViaJNI(keyExpr)

        @Throws(ZError::class)
        fun autocanonize(keyExpr: String): String = JNIKeyExprNative.autocanonizeViaJNI(keyExpr)

        @Throws(ZError::class)
        fun intersects(a: JNIKeyExpr?, aStr: String, b: JNIKeyExpr?, bStr: String): Boolean =
            JNIKeyExprNative.intersectsViaJNI(of(a, aStr), of(b, bStr))

        @Throws(ZError::class)
        fun includes(a: JNIKeyExpr?, aStr: String, b: JNIKeyExpr?, bStr: String): Boolean =
            JNIKeyExprNative.includesViaJNI(of(a, aStr), of(b, bStr))

        /** Returns SetIntersectionLevel ordinal as Int. Callers convert to SetIntersectionLevel. */
        @Throws(ZError::class)
        fun relationTo(a: JNIKeyExpr?, aStr: String, b: JNIKeyExpr?, bStr: String): Int =
            JNIKeyExprNative.relationToViaJNI(of(a, aStr), of(b, bStr))

        @Throws(ZError::class)
        fun join(a: JNIKeyExpr?, aStr: String, other: String): String =
            JNIKeyExprNative.joinViaJNI(of(a, aStr), other)

        @Throws(ZError::class)
        fun concat(a: JNIKeyExpr?, aStr: String, other: String): String =
            JNIKeyExprNative.concatViaJNI(of(a, aStr), other)
    }

    fun close() {
        // Guard against a future caller invoking close() on an undeclared
        // holder (ptr == 0L); the native side reconstructs `Arc::from_raw`,
        // which is UB on a null pointer.
        if (ptr != 0L) {
            JNIKeyExprNative.dropKeyExprViaJNI(ptr)
        }
    }
}
