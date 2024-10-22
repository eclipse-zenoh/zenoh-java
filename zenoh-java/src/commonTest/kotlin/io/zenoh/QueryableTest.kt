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
import io.zenoh.qos.CongestionControl
import io.zenoh.qos.Priority
import io.zenoh.qos.QoS
import io.zenoh.sample.SampleKind
import io.zenoh.query.Reply
import io.zenoh.queryable.Query
import io.zenoh.sample.Sample
import io.zenoh.value.Value
import org.apache.commons.net.ntp.TimeStamp
import java.time.Duration
import java.time.Instant
import java.util.*
import java.util.concurrent.BlockingQueue
import kotlin.test.*

class QueryableTest {

    companion object {
        val TEST_KEY_EXP = "example/testing/keyexpr".intoKeyExpr()
        const val testPayload = "Hello queryable"
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

    /** Test validating both Queryable and get operations. */
    @Test
    fun queryable_runsWithCallback() {
        val sample = Sample(
            testKeyExpr, Value(testPayload), SampleKind.PUT, TimeStamp(Date.from(Instant.now())), QoS.default()
        )
        val queryable = session.declareQueryable(testKeyExpr).with { query ->
            query.reply(testKeyExpr).success(sample.value).timestamp(sample.timestamp!!).res()
        }.res()

        var reply: Reply? = null
        val delay = Duration.ofMillis(1000)
        session.get(testKeyExpr).with { reply = it }.timeout(delay).res()

        Thread.sleep(1000)

        assertTrue(reply is Reply.Success)
        assertEquals((reply as Reply.Success).sample, sample)

        queryable.close()
    }

    @Test
    fun queryable_runsWithHandler() {
        val handler = QueryHandler()
        val queryable = session.declareQueryable(testKeyExpr).with(handler).res()

        val receivedReplies = ArrayList<Reply>()
        session.get(testKeyExpr).with { reply: Reply ->
            receivedReplies.add(reply)
        }.res()

        Thread.sleep(500)

        queryable.close()
        assertTrue(receivedReplies.all { it is Reply.Success })
        assertEquals(handler.performedReplies.size, receivedReplies.size)
    }

    @Test
    fun queryableBuilder_queueHandlerIsTheDefaultHandler() {
        val queryable = session.declareQueryable(TEST_KEY_EXP).res()
        assertTrue(queryable.receiver is BlockingQueue<Optional<Query>>)
        queryable.close()
    }

    @Test
    fun queryTest() {
        var receivedQuery: Query? = null
        val queryable = session.declareQueryable(testKeyExpr).with { query -> receivedQuery = query }.res()

        session.get(testKeyExpr).res()

        Thread.sleep(1000)
        queryable.close()
        assertNotNull(receivedQuery)
        assertNull(receivedQuery!!.value)
    }

    @Test
    fun queryWithValueTest() {
        var receivedQuery: Query? = null
        val queryable = session.declareQueryable(testKeyExpr).with { query -> receivedQuery = query }.res()

        session.get(testKeyExpr).withValue("Test value").res()

        Thread.sleep(1000)
        queryable.close()
        assertNotNull(receivedQuery)
        assertEquals(Value("Test value"), receivedQuery!!.value)
    }

    @Test
    fun queryReplySuccessTest() {
        val message = "Test message"
        val timestamp = TimeStamp.getCurrentTime()
        val priority = Priority.DATA_HIGH
        val express = true
        val congestionControl = CongestionControl.DROP
        val queryable = session.declareQueryable(testKeyExpr).with {
            it.use { query ->
                query.reply(testKeyExpr).success(message).timestamp(timestamp).priority(priority).express(express)
                    .congestionControl(congestionControl).res()
            }
        }.res()

        var receivedReply: Reply? = null
        session.get(testKeyExpr).with { receivedReply = it }.timeout(Duration.ofMillis(10)).res()

        queryable.close()

        assertTrue(receivedReply is Reply.Success)
        val reply = receivedReply as Reply.Success
        assertEquals(message, reply.sample.value.payload.decodeToString())
        assertEquals(timestamp, reply.sample.timestamp)
        assertEquals(priority, reply.sample.qos.priority)
        assertEquals(express, reply.sample.qos.express)
        assertEquals(congestionControl, reply.sample.qos.congestionControl)
    }

    @Test
    fun queryReplyErrorTest() {
        val message = "Error message"
        val queryable = session.declareQueryable(testKeyExpr).with {
            it.use { query ->
                query.reply(testKeyExpr).error(Value(message)).res()
            }
        }.res()

        var receivedReply: Reply? = null
        session.get(testKeyExpr).with { receivedReply = it }.timeout(Duration.ofMillis(10)).res()

        Thread.sleep(1000)
        queryable.close()

        assertNotNull(receivedReply)
        assertTrue(receivedReply is Reply.Error)
        val reply = receivedReply as Reply.Error
        assertEquals(message, reply.error.payload.decodeToString())
    }

    @Test
    fun queryReplyDeleteTest() {
        val timestamp = TimeStamp.getCurrentTime()
        val priority = Priority.DATA_HIGH
        val express = true
        val congestionControl = CongestionControl.DROP
        val queryable = session.declareQueryable(testKeyExpr).with {
            it.use { query ->
                query.reply(testKeyExpr).delete().timestamp(timestamp).priority(priority).express(express)
                    .congestionControl(congestionControl).res()
            }
        }.res()

        var receivedReply: Reply? = null
        session.get(testKeyExpr).with { receivedReply = it }.timeout(Duration.ofMillis(10)).res()

        queryable.close()

        assertNotNull(receivedReply)
        assertTrue(receivedReply is Reply.Delete)
        val reply = receivedReply as Reply.Delete
        assertEquals(timestamp, reply.timestamp)
        assertEquals(priority, reply.qos.priority)
        assertEquals(express, reply.qos.express)
        assertEquals(congestionControl, reply.qos.congestionControl)
    }

    @Test
    fun onCloseTest() {
        var onCloseWasCalled = false
        val queryable = session.declareQueryable(testKeyExpr).onClose { onCloseWasCalled = true }.res()
        queryable.undeclare()

        assertTrue(onCloseWasCalled)
    }
}

/** A dummy handler that replies "Hello queryable" followed by the count of replies performed. */
private class QueryHandler : Handler<Query, QueryHandler> {

    private var counter = 0

    val performedReplies: ArrayList<Sample> = ArrayList()

    override fun handle(t: Query) {
        reply(t)
    }

    override fun receiver(): QueryHandler {
        return this
    }

    override fun onClose() {}

    fun reply(query: Query) {
        val payload = "Hello queryable $counter!"
        counter++
        val sample = Sample(
            query.keyExpr, Value(payload), SampleKind.PUT, TimeStamp(Date.from(Instant.now())), QoS.default()
        )
        performedReplies.add(sample)
        query.reply(query.keyExpr).success(sample.value).timestamp(sample.timestamp!!).res()
    }
}
