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

package io.zenoh

import io.zenoh.bytes.ZBytes
import io.zenoh.exceptions.ZError
import io.zenoh.exceptions.throwZError0
import io.zenoh.session.SessionDeclaration
import java.util.concurrent.CyclicBarrier
import io.zenoh.jni.bytes.ZBytes as JniZBytes
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The explicit lifecycle of a received payload: it owns a native buffer until
 * its bytes are materialized (or discarded), and — unlike every other
 * handle-owning class here — has no garbage-collection backstop, so
 * [ZBytes.discard]/[ZBytes.close] is the only way to release one that is never
 * read. A handle-backed instance is built here directly rather than through a
 * session: what is under test is the wrapper's state machine, not delivery.
 */
class ZBytesLifecycleTest {

    private val payload = "the payload".encodeToByteArray()

    private fun received(): ZBytes = ZBytes.fromHandle(JniZBytes.newFromVec(payload, throwZError0))

    @Test
    fun discardBeforeMaterializationReleasesTheBufferAndInvalidatesTheBytes() {
        val zbytes = received()
        zbytes.discard()

        assertThrows(ZError::class.java) { zbytes.toBytes() }
    }

    @Test
    fun discardAfterMaterializationKeepsTheBytesReadable() {
        val zbytes = received()
        assertArrayEquals(payload, zbytes.toBytes())

        zbytes.discard()

        assertArrayEquals(payload, zbytes.toBytes())
    }

    @Test
    fun discardAndCloseAreIdempotent() {
        val unread = received()
        unread.close()
        unread.close()
        unread.discard()

        val read = received()
        read.toBytes()
        read.close()
        read.discard()
    }

    @Test
    fun closingAValueZBytesIsANoOp() {
        val zbytes = ZBytes.from(payload)
        zbytes.close()

        assertArrayEquals(payload, zbytes.toBytes())
    }

    @Test
    fun concurrentReadAndCloseHaveDeterministicOutcomes() {
        repeat(500) {
            val zbytes = received()
            val barrier = CyclicBarrier(2)
            var failure: Throwable? = null

            val reader = Thread {
                barrier.await()
                try {
                    assertArrayEquals(payload, zbytes.toBytes())
                } catch (e: ZError) {
                    // The discard won the race: a documented outcome.
                } catch (e: Throwable) {
                    failure = e
                }
            }
            val closer = Thread {
                barrier.await()
                try {
                    zbytes.close()
                } catch (e: Throwable) {
                    failure = e
                }
            }

            reader.start()
            closer.start()
            reader.join()
            closer.join()

            failure?.let { throw it }
        }
    }

    @Test
    fun aSessionDeclarationIsAutoCloseable() {
        assertTrue(AutoCloseable::class.java.isAssignableFrom(SessionDeclaration::class.java))
    }
}
