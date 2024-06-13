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

import io.zenoh.handlers.Handler
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.keyexpr.intoKeyExpr
import io.zenoh.prelude.CongestionControl
import io.zenoh.prelude.Encoding
import io.zenoh.prelude.Priority
import io.zenoh.sample.Sample
import io.zenoh.value.Value
import java.util.*
import java.util.concurrent.BlockingQueue
import kotlin.collections.ArrayDeque
import kotlin.collections.ArrayList
import kotlin.test.*

class SubscriberTest {

    companion object {
        val TEST_PRIORITY = Priority.DATA_HIGH;
        val TEST_CONGESTION_CONTROL = CongestionControl.BLOCK;

        val testValues = arrayListOf(
            Value("Test 1".encodeToByteArray(), Encoding(Encoding.ID.TEXT_PLAIN)),
            Value("Test 2".encodeToByteArray(), Encoding(Encoding.ID.TEXT_JSON)),
            Value("Test 3".encodeToByteArray(), Encoding(Encoding.ID.TEXT_CSV))
        )
    }

    private lateinit var session: Session
    private lateinit var testKeyExpr: KeyExpr

    @BeforeTest
    fun setUp() {
        session = Session.open()
        testKeyExpr = "example/testing/keyexpr".intoKeyExpr()
    }

    @AfterTest
    fun tearDown() {
        session.close()
        testKeyExpr.close()
    }

    @Test
    fun subscriber_runsWithCallback() {
        val receivedSamples = ArrayList<Sample>()
        val subscriber =
            session.declareSubscriber(testKeyExpr).with { sample -> receivedSamples.add(sample) }.res()

        testValues.forEach { value ->
            session.put(testKeyExpr, value)
                .priority(TEST_PRIORITY)
                .congestionControl(TEST_CONGESTION_CONTROL)
                .res()
        }
        assertEquals(receivedSamples.size, testValues.size)

        receivedSamples.zip(testValues).forEach { (sample, value) ->
            assertEquals(sample.value, value)
            assertEquals(sample.qos.priority(), TEST_PRIORITY)
            assertEquals(sample.qos.congestionControl(), TEST_CONGESTION_CONTROL)
        }

        subscriber.close()
    }

    @Test
    fun subscriber_runsWithHandler() {
        val handler = QueueHandler<Sample>()
        val subscriber = session.declareSubscriber(testKeyExpr).with(handler).res()

        testValues.forEach { value ->
            session.put(testKeyExpr, value)
                .priority(TEST_PRIORITY)
                .congestionControl(TEST_CONGESTION_CONTROL)
                .res()
        }
        assertEquals(handler.queue.size, testValues.size)

        handler.queue.zip(testValues).forEach { (sample, value) ->
            assertEquals(sample.value, value)
            assertEquals(sample.qos.priority(), TEST_PRIORITY)
            assertEquals(sample.qos.congestionControl(), TEST_CONGESTION_CONTROL)
        }

        subscriber.close()
    }

    @Test
    fun subscriberBuilder_queueHandlerIsTheDefaultHandler() {
        val subscriber = session.declareSubscriber(testKeyExpr).res()
        subscriber.close()
        assertTrue(subscriber.receiver is BlockingQueue<Optional<Sample>>)
    }

    @Test
    fun subscriber_isDeclaredWithNonDeclaredKeyExpression() {
        // Declaring a subscriber with an undeclared key expression and verifying it properly receives samples.
        val keyExpr = KeyExpr("example/**")
        val session = Session.open()

        val receivedSamples = ArrayList<Sample>()
        val subscriber = session.declareSubscriber(keyExpr).with { sample -> receivedSamples.add(sample) }.res()
        testValues.forEach { value -> session.put(testKeyExpr, value).res() }
        subscriber.close()

        assertEquals(receivedSamples.size, testValues.size)

        for ((index, sample) in receivedSamples.withIndex()) {
            assertEquals(sample.value, testValues[index])
        }
    }

    @Test
    fun onCloseTest() {
        var onCloseWasCalled = false
        val subscriber = session.declareSubscriber(testKeyExpr).onClose { onCloseWasCalled = true }.res()
        subscriber.undeclare()
        assertTrue(onCloseWasCalled)
    }
}

private class QueueHandler<T : ZenohType> : Handler<T, ArrayDeque<T>> {

    val queue: ArrayDeque<T> = ArrayDeque()
    override fun handle(t: T) {
        queue.add(t)
    }

    override fun receiver(): ArrayDeque<T> {
        return queue
    }

    override fun onClose() {}
}
