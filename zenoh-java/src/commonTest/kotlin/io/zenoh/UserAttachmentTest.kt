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

import io.zenoh.jni.decodeAttachment
import io.zenoh.jni.encodeAttachment
import io.zenoh.jni.toByteArray
import io.zenoh.jni.toInt
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.keyexpr.intoKeyExpr
import io.zenoh.prelude.Encoding
import io.zenoh.prelude.KnownEncoding
import io.zenoh.query.Reply
import io.zenoh.sample.Attachment
import io.zenoh.sample.Sample
import io.zenoh.value.Value
import java.time.Duration
import kotlin.test.*


class UserAttachmentTest {

    private lateinit var session: Session
    private lateinit var keyExpr: KeyExpr

    companion object {
        val value = Value("test", Encoding(KnownEncoding.TEXT_PLAIN))
        const val keyExprString = "example/testing/attachment"
        val attachmentPairs = arrayListOf(
            "key1" to "value1", "key2" to "value2", "key3" to "value3", "repeatedKey" to "value1", "repeatedKey" to "value2"
        )
        val attachment =
            Attachment(attachmentPairs.map { it.first.encodeToByteArray() to it.second.encodeToByteArray() })
    }

    @BeforeTest
    fun setup() {
        session = Session.open()
        keyExpr = keyExprString.intoKeyExpr()
    }

    @AfterTest
    fun tearDown() {
        session.close()
        keyExpr.close()
    }

    private fun assertAttachmentOk(attachment: Attachment?) {
        assertNotNull(attachment)
        val receivedPairs = attachment.values
        assertEquals(attachmentPairs.size, receivedPairs.size)
        for ((index, receivedPair) in receivedPairs.withIndex()) {
            assertEquals(attachmentPairs[index].first, receivedPair.first.decodeToString())
            assertEquals(attachmentPairs[index].second, receivedPair.second.decodeToString())
        }
    }

    @Test
    fun putWithAttachmentTest() {
        var receivedSample: Sample? = null
        val subscriber = session.declareSubscriber(keyExpr).with { sample -> receivedSample = sample }.res()
        session.put(keyExpr, value).withAttachment(attachment).res()

        subscriber.close()

        assertNotNull(receivedSample) {
            assertEquals(value, it.value)
            assertAttachmentOk(it.attachment)
        }
    }

    @Test
    fun publisherPutWithAttachmentTest() {
        var receivedSample: Sample? = null
        val publisher = session.declarePublisher(keyExpr).res()
        val subscriber = session.declareSubscriber(keyExpr).with { sample ->
            receivedSample = sample
        }.res()

        publisher.put("test").withAttachment(attachment).res()

        publisher.close()
        subscriber.close()

        assertAttachmentOk(receivedSample!!.attachment!!)
    }

    @Test
    fun publisherPutWithoutAttachmentTest() {
        var receivedSample: Sample? = null
        val publisher = session.declarePublisher(keyExpr).res()
        val subscriber = session.declareSubscriber(keyExpr).with { sample -> receivedSample = sample }.res()

        publisher.put("test").res()

        publisher.close()
        subscriber.close()

        assertNotNull(receivedSample) {
            assertNull(it.attachment)
        }
    }

    @Test
    fun publisherDeleteWithAttachmentTest() {
        var receivedSample: Sample? = null
        val publisher = session.declarePublisher(keyExpr).res()
        val subscriber = session.declareSubscriber(keyExpr).with { sample -> receivedSample = sample }.res()

        publisher.delete().withAttachment(attachment).res()

        publisher.close()
        subscriber.close()

        assertAttachmentOk(receivedSample?.attachment)
    }

    @Test
    fun publisherDeleteWithoutAttachmentTest() {
        var receivedSample: Sample? = null
        val publisher = session.declarePublisher(keyExpr).res()
        val subscriber = session.declareSubscriber(keyExpr).with { sample -> receivedSample = sample }.res()

        publisher.delete().res()

        publisher.close()
        subscriber.close()

        assertNotNull(receivedSample) {
            assertNull(it.attachment)
        }
    }

    @Test
    fun queryWithAttachmentTest() {
        var receivedAttachment: Attachment? = null
        val queryable = session.declareQueryable(keyExpr).with { query ->
            receivedAttachment = query.attachment
            query.reply(keyExpr).success("test").res()
        }.res()

        session.get(keyExpr).with {}.withAttachment(attachment).timeout(Duration.ofMillis(1000)).res()

        queryable.close()

        assertAttachmentOk(receivedAttachment)
    }

    @Test
    fun queryReplyWithAttachmentTest() {
        var receivedAttachment: Attachment? = null
        val queryable = session.declareQueryable(keyExpr).with { query ->
            query.reply(keyExpr).success("test").withAttachment(attachment).res()
        }.res()

        session.get(keyExpr).with { reply ->
            if (reply is Reply.Success) {
                receivedAttachment = reply.sample.attachment
            }
        }.timeout(Duration.ofMillis(1000)).res()

        queryable.close()

        assertAttachmentOk(receivedAttachment)
    }

    @Test
    fun queryReplyWithoutAttachmentTest() {
        var reply: Reply? = null
        val queryable = session.declareQueryable(keyExpr).with { query ->
            query.reply(keyExpr).success("test").res()
        }.res()

        session.get(keyExpr).with {
            reply = it
        }.timeout(Duration.ofMillis(1000)).res()

        queryable.close()

        assertNotNull(reply) {
            assertTrue(it is Reply.Success)
            assertNull(it.sample.attachment)
        }
    }

    @Test
    fun encodeAndDecodeNumbersTest() {
        val numbers: List<Int> = arrayListOf(0, 1, -1, 12345, -12345, 123567, 123456789, -123456789)

        numbers.forEach { number ->
            val bytes = number.toByteArray()
            val decodedNumber: Int = bytes.toInt()
            assertEquals(number, decodedNumber)
        }
    }

    @Test
    fun encodeAndDecodeAttachmentTest() {
        val encodedAttachment = encodeAttachment(attachment)
        val decodedAttachment = decodeAttachment(encodedAttachment)

        assertAttachmentOk(decodedAttachment)
    }
}
