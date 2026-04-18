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
import kotlin.reflect.KType

/**
 * JNI bridge for Kotlin-type-aware serialization/deserialization.
 *
 * Uses [KType] (obtained via `typeOf<T>()` with reified generics) instead of
 * [java.lang.reflect.Type], making this bridge usable from commonMain Kotlin
 * code and compatible with Kotlin-specific types (unsigned integers, Pair, Triple).
 *
 * Supported types:
 * - Primitives: Boolean, Byte, Short, Int, Long, Float, Double
 * - Unsigned (Kotlin-only): UByte, UShort, UInt, ULong
 * - Text/Binary: String, ByteArray
 * - Collections: List<T>, Map<K, V> (recursive)
 * - Tuples: Pair<A, B>, Triple<A, B, C>
 */
object JNIZBytesKotlin {

    init {
        ZenohLoad
    }

    fun serialize(any: Any, kType: KType): ByteArray = serializeViaJNI(any, kType)

    fun deserialize(bytes: ByteArray, kType: KType): Any = deserializeViaJNI(bytes, kType)

    @JvmStatic
    private external fun serializeViaJNI(any: Any, kType: KType): ByteArray

    @JvmStatic
    private external fun deserializeViaJNI(bytes: ByteArray, kType: KType): Any
}
