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

package io.zenoh

import io.zenoh.config.ZenohId
import io.zenoh.exceptions.throwZError0
import io.zenoh.jni.config.ZenohId as JniZenohId
import kotlin.random.Random
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Correspondence test for the pure-JVM [ZenohId.toString] — the id bytes read
 * as a little-endian integer, rendered as lowercase hex without leading zeros
 * (`"0"` for the zero id). Verified against the native formatter
 * (`zenoh_id_to_string`) over edge patterns and a deterministic random corpus.
 */
class ZenohIdCorrespondenceTest {

    private fun assertCorresponds(bytes: ByteArray) {
        val jni = JniZenohId(bytes)
        assertEquals(
            "zid render mismatch for ${bytes.joinToString(",")}",
            jni.toStr(throwZError0),
            ZenohId(jni).toString(),
        )
    }

    @Test
    fun rendersLikeNative() {
        val edges = listOf(
            ByteArray(16), // zero id
            ByteArray(16).also { it[0] = 1 }, // smallest nonzero (LE low byte)
            ByteArray(16).also { it[15] = 1 }, // highest byte only
            ByteArray(16).also { it[0] = 0x0F }, // sub-0x10 low byte
            ByteArray(16) { 0xFF.toByte() }, // all ones
            ByteArray(16) { i -> i.toByte() }, // ascending with leading zero byte
        )
        edges.forEach(::assertCorresponds)

        val rng = Random(20260716)
        repeat(32) {
            assertCorresponds(rng.nextBytes(16))
        }
    }
}
