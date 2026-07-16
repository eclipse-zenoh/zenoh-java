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
 * End-to-end test of the publisher's **default encoding**: set natively once
 * at declare time ([PublisherOptions.encoding]), applied by zenoh itself to
 * every plain `put` — which crosses **no encoding data at all** — and
 * overridden per call via [PutOptions.encoding].
 */
class PublisherEncodingTest {

    private val custom = Encoding.from("application/custom;my-schema")

    @Test
    fun publisherDefaultAppliesNativelyAndPerPutOverrides() {
        val session = Zenoh.open(Config.loadDefault())
        val keyExpr = KeyExpr.tryFrom("example/testing/publisher/encoding")
        val received = mutableListOf<Sample>()
        val subscriber = session.declareSubscriber(keyExpr) { received.add(it) }

        val options = PublisherOptions()
        options.encoding = custom
        val publisher = session.declarePublisher(keyExpr, options)

        // Plain put: NO encoding crosses — the native publisher default applies.
        publisher.put(ZBytes.from("default"))
        // Per-put override still works.
        val putOptions = PutOptions()
        putOptions.encoding = Encoding.TEXT_PLAIN
        publisher.put(ZBytes.from("override"), putOptions)
        Thread.sleep(500)

        publisher.close()
        subscriber.close()
        session.close()
        assertEquals(2, received.size)
        assertEquals(custom, received[0].encoding)
        assertEquals(Encoding.TEXT_PLAIN, received[1].encoding)
    }
}
