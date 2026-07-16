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

import io.zenoh.bytes.Encoding
import io.zenoh.bytes.ZBytes
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.pubsub.PublisherOptions
import io.zenoh.pubsub.PutOptions
import io.zenoh.sample.Sample
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * End-to-end tests of the two encoding fast paths:
 *  * the publisher's **default encoding**, set natively once at declare time
 *    (plain `put`s cross no encoding data at all), and
 *  * a **pinned** encoding ([Encoding.pinned]) passed per call as a
 *    preallocated native handle (no schema-string crossing per put).
 */
class PinnedEncodingTest {

    private val custom = Encoding.from("application/custom;my-schema")

    @Test
    fun publisherDefaultEncodingAppliesNatively() {
        val session = Zenoh.open(Config.loadDefault())
        val keyExpr = KeyExpr.tryFrom("example/testing/pinned/default")
        val received = mutableListOf<Sample>()
        val subscriber = session.declareSubscriber(keyExpr) { received.add(it) }

        val options = PublisherOptions()
        options.encoding = custom
        val publisher = session.declarePublisher(keyExpr, options)
        // Plain put: NO encoding crosses — the native publisher default applies.
        publisher.put(ZBytes.from("hello"))
        Thread.sleep(500)

        publisher.close()
        subscriber.close()
        session.close()
        assertEquals(1, received.size)
        assertEquals(custom, received[0].encoding)
    }

    @Test
    fun pinnedEncodingOverridesPerPut() {
        val session = Zenoh.open(Config.loadDefault())
        val keyExpr = KeyExpr.tryFrom("example/testing/pinned/override")
        val received = mutableListOf<Sample>()
        val subscriber = session.declareSubscriber(keyExpr) { received.add(it) }
        val publisher = session.declarePublisher(keyExpr)

        val pinned = custom.pinned()
        val options = PutOptions()
        options.encoding = pinned
        // The pinned handle is BORROWED per call — reusable across the loop.
        repeat(3) { publisher.put(ZBytes.from("hello #$it"), options) }
        Thread.sleep(500)
        assertEquals(3, received.size)
        received.forEach { assertEquals(custom, it.encoding) }

        // After close() the pinned form falls back to the plain (id, schema)
        // crossing — same value, still correct.
        pinned.close()
        publisher.put(ZBytes.from("after close"), options)
        Thread.sleep(500)

        publisher.close()
        subscriber.close()
        session.close()
        assertEquals(4, received.size)
        assertEquals(custom, received[3].encoding)
    }
}
