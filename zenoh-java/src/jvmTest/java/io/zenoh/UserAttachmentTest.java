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
import io.zenoh.pubsub.DeleteOptions;
import io.zenoh.pubsub.PutOptions;
import io.zenoh.pubsub.Subscriber;
import io.zenoh.query.GetOptions;
import io.zenoh.query.Reply;
import io.zenoh.query.ReplyOptions;
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
        Subscriber<Void> subscriber =
            session.declareSubscriber(keyExpr, sample -> receivedSample[0] = sample);

        var putOptions = new PutOptions();
        putOptions.setAttachment(attachment);
        session.put(keyExpr, payload, putOptions);

        subscriber.close();

        assertNotNull(receivedSample[0]);
        ZBytes receivedAttachment = receivedSample[0].getAttachment();
        assertEquals(attachment, receivedAttachment);
    }

    @Test
    public void publisherPutWithAttachmentTest() throws ZError {
        Sample[] receivedSample = new Sample[1];
        var publisher = session.declarePublisher(keyExpr);
        Subscriber<Void> subscriber =
                session.declareSubscriber(keyExpr, sample -> receivedSample[0] = sample);

        var putOptions = new PutOptions();
        putOptions.setAttachment(attachment);
        publisher.put(payload, putOptions);

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
        Subscriber<Void> subscriber =
                session.declareSubscriber(keyExpr, sample -> receivedSample[0] = sample);

        publisher.put(payload);

        publisher.close();
        subscriber.close();

        assertNotNull(receivedSample[0]);
        assertNull(receivedSample[0].getAttachment());
    }

    @Test
    public void publisherDeleteWithAttachmentTest() throws ZError {
        Sample[] receivedSample = new Sample[1];
        var publisher = session.declarePublisher(keyExpr);
        Subscriber<Void> subscriber =
                session.declareSubscriber(keyExpr, sample -> receivedSample[0] = sample);

        var deleteOptions = new DeleteOptions();
        deleteOptions.setAttachment(attachment);
        publisher.delete(deleteOptions);

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
        Subscriber<Void> subscriber =
                session.declareSubscriber(keyExpr, sample -> receivedSample[0] = sample);

        publisher.delete();

        publisher.close();
        subscriber.close();

        assertNotNull(receivedSample[0]);
        assertNull(receivedSample[0].getAttachment());
    }

    @Test
    public void queryWithAttachmentTest() throws ZError {
        ZBytes[] receivedAttachment = new ZBytes[1];
        var queryable = session.declareQueryable(keyExpr, query -> {
            receivedAttachment[0] = query.getAttachment();
            try {
                query.reply(keyExpr, payload);
            } catch (ZError e) {
                throw new RuntimeException(e);
            }
        });

        var getOptions = new GetOptions();
        getOptions.setTimeout(Duration.ofMillis(1000));
        getOptions.setAttachment(attachment);
        session.get(keyExpr, getOptions);

        queryable.close();

        assertNotNull(receivedAttachment[0]);
        assertEquals(attachment, receivedAttachment[0]);
    }

    @Test
    public void queryReplyWithAttachmentTest() throws ZError {
        Reply[] reply = new Reply[1];
        var queryable = session.declareQueryable(keyExpr, query -> {
            try {
                var options = new ReplyOptions();
                options.setAttachment(attachment);
                query.reply(keyExpr, payload, options);
            } catch (ZError e) {
                throw new RuntimeException(e);
            }
        });


        var getOptions = new GetOptions();
        getOptions.setTimeout(Duration.ofMillis(1000));
        getOptions.setAttachment(attachment);
        session.get(keyExpr, reply1 -> reply[0] = reply1, getOptions);

        queryable.close();

        Reply receivedReply = reply[0];
        assertNotNull(receivedReply);
        ZBytes receivedAttachment = ((Reply.Success) receivedReply).getSample().getAttachment();
        assertEquals(attachment, receivedAttachment);
    }

    @Test
    public void queryReplyWithoutAttachmentTest() throws ZError {
        Reply[] reply = new Reply[1];
        var queryable = session.declareQueryable(keyExpr, query -> {
            try {
                query.reply(keyExpr, payload);
            } catch (ZError e) {
                throw new RuntimeException(e);
            }
        });
        session.get(keyExpr, reply1 -> reply[0] = reply1);

        queryable.close();

        Reply receivedReply = reply[0];
        assertNotNull(receivedReply);
        ZBytes receivedAttachment = ((Reply.Success) receivedReply).getSample().getAttachment();
        assertNull(receivedAttachment);
    }
}
