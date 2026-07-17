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
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.sample.Sample
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The key-expression ownership model: a native handle exists ONLY behind
 * `Session.declareKeyExpr` — the single case zenoh attaches a wire
 * declaration worth carrying. Everything else — `tryFrom`, `autocanonize`,
 * and every RECEIVED keyexpr (zenoh's RX path never attaches a declaration)
 * — is a plain string-backed value: nothing to close, no native resource,
 * no per-message allocation or leak.
 */
class KeyExprHandleTest {

    @Test
    fun constructedKeyExprsAreStringBacked() {
        val ke = KeyExpr.tryFrom("example/testing/keyexpr")
        assertNull(ke.handle)
        assertEquals("example/testing/keyexpr", ke.toString())
        // Canonization still happens natively; the result keeps the string.
        val canon = KeyExpr.autocanonize("example/**/**")
        assertNull(canon.handle)
        assertEquals("example/**", canon.toString())
        // Algebra ops work on string-backed instances (transient native probe).
        assertTrue(ke.intersects(KeyExpr.tryFrom("example/testing/*")))
        assertTrue(KeyExpr.tryFrom("example/**").includes(ke))
        assertEquals("example/testing/keyexpr/sub", ke.join("sub").toString())
    }

    @Test
    fun declaredKeyExprOwnsAHandleUntilUndeclared() {
        val session = Zenoh.open(Config.loadDefault())
        val declared = session.declareKeyExpr("example/testing/keyexpr/declared")
        assertNotNull(declared.handle)
        // Ops route through the declared handle.
        assertTrue(declared.intersects(KeyExpr.tryFrom("example/testing/**")))
        // Undeclare demotes to string-backed; the string survives.
        session.undeclare(declared)
        assertNull(declared.handle)
        assertEquals("example/testing/keyexpr/declared", declared.toString())
        session.close()
    }

    @Test
    fun failedUndeclarationStillDetachesTheConsumedHandle() {
        // Undeclaring through the WRONG session makes the native undeclare
        // fail — and the generated wrapper consumes the handle even then. The
        // KeyExpr must degrade to its string form (handle detached), not keep
        // selecting a dead handle.
        val session1 = Zenoh.open(Config.loadDefault())
        val session2 = Zenoh.open(Config.loadDefault())
        val declared = session1.declareKeyExpr("example/testing/keyexpr/wrongsession")
        var failed = false
        try {
            session2.undeclare(declared)
        } catch (e: ZError) {
            failed = true
        }
        assertTrue(failed)
        assertNull(declared.handle)
        // String-backed operation keeps working after the failed undeclare.
        assertTrue(declared.intersects(KeyExpr.tryFrom("example/testing/**")))
        session1.put(declared, ZBytes.from("test"))
        session1.close()
        session2.close()
    }

    @Test
    fun receivedKeyExprIsStringBackedAndReusable() {
        val session = Zenoh.open(Config.loadDefault())
        val keyExpr = KeyExpr.tryFrom("example/testing/keyexpr/received")
        val received = mutableListOf<Sample>()
        val subscriber = session.declareSubscriber(keyExpr) { received.add(it) }

        session.put(keyExpr, ZBytes.from("one"))
        Thread.sleep(500)

        assertEquals(1, received.size)
        val saved = received[0].keyExpr
        // Delivered as ONE eager string: no native allocation, nothing to
        // free — a received keyexpr never carries a wire declaration, so a
        // handle would buy nothing on re-send.
        assertNull(saved.handle)
        assertEquals(keyExpr, saved)

        // The user scenario: re-publish with the SAVED keyexpr.
        session.put(saved, ZBytes.from("two"))
        Thread.sleep(500)
        assertEquals(2, received.size)
        assertEquals(saved, received[1].keyExpr)

        subscriber.close()
        session.close()
    }
}
