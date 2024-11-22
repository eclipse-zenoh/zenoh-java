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

package io.zenoh;

import io.zenoh.exceptions.ZError;
import io.zenoh.keyexpr.KeyExpr;
import io.zenoh.bytes.ZBytes;
import io.zenoh.pubsub.Subscriber;
import io.zenoh.pubsub.SubscriberConfig;
import io.zenoh.query.Reply;
import io.zenoh.sample.Sample;
import org.junit.After;
import org.junit.Before;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.junit.runners.JUnit4;

import java.time.Duration;

import static org.junit.Assert.*;

@RunWith(JUnit4.class)
public class UserAttachmentTest {

    static final KeyExpr keyExpr;
    static final ZBytes payload = ZBytes.from("test payload");
    static final ZBytes attachment = ZBytes.from("mock_attachment");
    static {
        try {
            keyExpr = KeyExpr.tryFrom("example/testing/attachment");
        } catch (ZError e) {
            throw new RuntimeException(e);
        }
    }

    Session session;

    @Before
    public void setup() throws ZError {
        session = Zenoh.open(Config.loadDefault());
    }

    @After
    public void tearDown() {
        session.close();
    }

    @Test
    public void putWithAttachmentTest() throws ZError {
        Sample[] receivedSample = new Sample[1];
        SubscriberConfig<Void> config = new SubscriberConfig<>();
        config.setCallback(sample -> receivedSample[0] = sample);
        Subscriber<Void> subscriber =
            session.declareSubscriber(keyExpr, config);

        session.put(keyExpr, payload).attachment(attachment).res();

        subscriber.close();

        assertNotNull(receivedSample[0]);
        ZBytes receivedAttachment = receivedSample[0].getAttachment();
        assertEquals(attachment, receivedAttachment);
    }

    @Test
    public void publisherPutWithAttachmentTest() throws ZError {
        Sample[] receivedSample = new Sample[1];
        var publisher = session.declarePublisher(keyExpr);
        SubscriberConfig<Void> config = new SubscriberConfig<>();
        config.setCallback(sample -> receivedSample[0] = sample);
        Subscriber<Void> subscriber =
                session.declareSubscriber(keyExpr, config);

        publisher.put(payload).attachment(attachment).res();

        publisher.close();
        subscriber.close();

        assertNotNull(receivedSample[0]);
        ZBytes receivedAttachment = receivedSample[0].getAttachment();
        assertEquals(attachment, receivedAttachment);
    }

    @Test
    public void publisherPutWithoutAttachmentTest() throws ZError {
        Sample[] receivedSample = new Sample[1];
        var publisher = session.declarePublisher(keyExpr);
        SubscriberConfig<Void> config = new SubscriberConfig<>();
        config.setCallback(sample -> receivedSample[0] = sample);
        Subscriber<Void> subscriber =
                session.declareSubscriber(keyExpr, config);

        publisher.put(payload).res();

        publisher.close();
        subscriber.close();

        assertNotNull(receivedSample[0]);
        assertNull(receivedSample[0].getAttachment());
    }

    @Test
    public void publisherDeleteWithAttachmentTest() throws ZError {
        Sample[] receivedSample = new Sample[1];
        var publisher = session.declarePublisher(keyExpr);
        SubscriberConfig<Void> config = new SubscriberConfig<>();
        config.setCallback(sample -> receivedSample[0] = sample);
        Subscriber<Void> subscriber =
                session.declareSubscriber(keyExpr, config);

        publisher.delete().attachment(attachment).res();

        publisher.close();
        subscriber.close();

        assertNotNull(receivedSample[0]);
        ZBytes receivedAttachment = receivedSample[0].getAttachment();
        assertEquals(attachment, receivedAttachment);
    }

    @Test
    public void publisherDeleteWithoutAttachmentTest() throws ZError {
        Sample[] receivedSample = new Sample[1];
        var publisher = session.declarePublisher(keyExpr);
        SubscriberConfig<Void> config = new SubscriberConfig<>();
        config.setCallback(sample -> receivedSample[0] = sample);
        Subscriber<Void> subscriber =
                session.declareSubscriber(keyExpr, config);

        publisher.delete().res();

        publisher.close();
        subscriber.close();

        assertNotNull(receivedSample[0]);
        assertNull(receivedSample[0].getAttachment());
    }

    @Test
    public void queryWithAttachmentTest() throws ZError {
        ZBytes[] receivedAttachment = new ZBytes[1];
        var queryable = session.declareQueryable(keyExpr).callback(query -> {
            receivedAttachment[0] = query.getAttachment();
            try {
                query.reply(keyExpr, payload).res();
            } catch (ZError e) {
                throw new RuntimeException(e);
            }
        }).res();

        session.get(keyExpr).callback(reply -> {}).attachment(attachment).timeout(Duration.ofMillis(1000)).res();

        queryable.close();

        assertNotNull(receivedAttachment[0]);
        assertEquals(attachment, receivedAttachment[0]);
    }

    @Test
    public void queryReplyWithAttachmentTest() throws ZError {
        Reply[] reply = new Reply[1];
        var queryable = session.declareQueryable(keyExpr).callback(query -> {
            try {
                query.reply(keyExpr, payload).attachment(attachment).res();
            } catch (ZError e) {
                throw new RuntimeException(e);
            }
        }).res();

        session.get(keyExpr).callback(reply1 -> reply[0] = reply1).attachment(attachment).timeout(Duration.ofMillis(1000)).res();

        queryable.close();

        Reply receivedReply = reply[0];
        assertNotNull(receivedReply);
        ZBytes receivedAttachment = ((Reply.Success) receivedReply).getSample().getAttachment();
        assertEquals(attachment, receivedAttachment);
    }

    @Test
    public void queryReplyWithoutAttachmentTest() throws ZError {
        Reply[] reply = new Reply[1];
        var queryable = session.declareQueryable(keyExpr).callback(query -> {
            try {
                query.reply(keyExpr, payload).res();
            } catch (ZError e) {
                throw new RuntimeException(e);
            }
        }).res();
        session.get(keyExpr).callback(reply1 -> reply[0] = reply1).timeout(Duration.ofMillis(1000)).res();

        queryable.close();

        Reply receivedReply = reply[0];
        assertNotNull(receivedReply);
        ZBytes receivedAttachment = ((Reply.Success) receivedReply).getSample().getAttachment();
        assertNull(receivedAttachment);
    }
}
