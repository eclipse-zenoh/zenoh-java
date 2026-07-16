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

import io.zenoh.bytes.Encoding
import io.zenoh.bytes.ZBytes
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.pubsub.PutOptions
import io.zenoh.sample.Sample
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Test

/**
 * The encoding native-handle ownership model, shaped to minimize JNI
 * crossings:
 *
 * - predefined constants are VALUE-ONLY — a send carries just their id inside
 *   the send call itself, no handle ever exists;
 * - custom (schema-carrying) encodings own a handle from construction —
 *   construction is the one crossing, every send after is a bare jlong;
 * - received encodings arrive WITH their handle in the same delivery
 *   crossing, so re-sending one (the save-and-republish scenario) never
 *   rebuilds the native value from its schema string.
 */
class EncodingHandleTest {

    @Test
    fun predefinedEncodingsAreValueOnly() {
        assertNull(Encoding.TEXT_PLAIN.handle)
        assertNull(Encoding.APPLICATION_JSON.handle)
        assertNull(Encoding.defaultEncoding().handle)
        // Parsing a plain well-known name yields the same value-only form.
        assertNull(Encoding.from("text/plain").handle)
    }

    @Test
    fun customEncodingsOwnAHandleFromConstruction() {
        assertNotNull(Encoding.from("text/plain;utf-8").handle)
        assertNotNull(Encoding.from("my_custom_encoding").handle)
        assertNotNull(Encoding.TEXT_PLAIN.withSchema("utf-8").handle)
    }

    @Test
    fun receivedEncodingIsSendReadyAndResendsByHandle() {
        val custom = Encoding.from("application/custom;my-schema")
        val session = Zenoh.open(Config.loadDefault())
        val keyExpr = KeyExpr.tryFrom("example/testing/encoding/handle")
        val received = mutableListOf<Sample>()
        val subscriber = session.declareSubscriber(keyExpr) { received.add(it) }

        // Send 1: user-created custom encoding — crosses by handle.
        val putOptions = PutOptions()
        putOptions.encoding = custom
        session.put(keyExpr, ZBytes.from("one"), putOptions)
        Thread.sleep(500)

        assertEquals(1, received.size)
        val saved = received[0].encoding
        assertEquals(custom, saved)
        // Delivered send-ready: the handle arrived with the sample.
        assertNotNull(saved.handle)

        // Send 2: the user's scenario — re-publish with the SAVED encoding.
        // The retained handle is what crosses; nothing is rebuilt.
        val handleBefore = saved.handle
        val resendOptions = PutOptions()
        resendOptions.encoding = saved
        session.put(keyExpr, ZBytes.from("two"), resendOptions)
        Thread.sleep(500)

        assertEquals(2, received.size)
        assertEquals(custom, received[1].encoding)
        // The saved encoding still owns the same reusable handle (borrowed,
        // never consumed, by the send).
        assertSame(handleBefore, saved.handle)
        assertNotNull(received[1].encoding.handle)

        subscriber.close()
        session.close()
    }

    @Test
    fun predefinedRoundTripStaysValueOnSendAndHandleOnReceive() {
        val session = Zenoh.open(Config.loadDefault())
        val keyExpr = KeyExpr.tryFrom("example/testing/encoding/preset")
        val received = mutableListOf<Sample>()
        val subscriber = session.declareSubscriber(keyExpr) { received.add(it) }

        val putOptions = PutOptions()
        putOptions.encoding = Encoding.TEXT_PLAIN
        session.put(keyExpr, ZBytes.from("plain"), putOptions)
        Thread.sleep(500)

        // The preset itself never grew a handle from being sent…
        assertNull(Encoding.TEXT_PLAIN.handle)
        assertEquals(1, received.size)
        assertEquals(Encoding.TEXT_PLAIN, received[0].encoding)
        // …while the received copy arrived send-ready.
        assertNotNull(received[0].encoding.handle)

        subscriber.close()
        session.close()
    }
}
