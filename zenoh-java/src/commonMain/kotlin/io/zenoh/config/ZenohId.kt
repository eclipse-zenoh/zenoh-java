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

package io.zenoh.config

import io.zenoh.jni.config.ZenohId as JniZenohId
import kotlin.math.absoluteValue

/**
 * The global unique id of a Zenoh peer.
 */
data class ZenohId internal constructor(internal val inner: JniZenohId) {

    /**
     * The standard string form: the id bytes read as a little-endian integer,
     * rendered as lowercase hex without leading zeros (`"0"` for the zero id) —
     * Zenoh's own rule (uhlc `ID`'s `Debug`), implemented in pure Kotlin over
     * the value-class bytes; correspondence with the native formatter is
     * verified by `ZenohIdCorrespondenceTest`.
     */
    override fun toString(): String {
        val hex = "0123456789abcdef"
        val sb = StringBuilder(inner.bytes.size * 2)
        for (i in inner.bytes.indices.reversed()) {
            val b = inner.bytes[i].toInt() and 0xFF
            sb.append(hex[b ushr 4]).append(hex[b and 0x0F])
        }
        val s = sb.trimStart('0').toString()
        return s.ifEmpty { "0" }
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (javaClass != other?.javaClass) return false

        other as ZenohId

        return inner.bytes.contentEquals(other.inner.bytes)
    }

    override fun hashCode(): Int {
        return inner.bytes.contentHashCode()
    }
}

/**
 * The global unique id of a Zenoh entity.
 * Contains two fields:
 * - zid: the global unique id of a Zenoh peer.
 * - eid: *unsigned* unique identifier of the entity within the Zenoh peer.
 */
data class EntityGlobalId internal constructor(
    val zid: ZenohId,
    // Rename default getter which is not accessible on Java due to unsigned type
    @get:JvmName("getEidUInt") val eid: UInt,
) {

    @Suppress("unused")
    // Manually defined getter for Java
    fun getEid(): Long = eid.toLong().absoluteValue
}
