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

import io.zenoh.bytes.Encoding;
import io.zenoh.bytes.ZBytes;
import io.zenoh.exceptions.ZError;
import io.zenoh.keyexpr.KeyExpr;
import io.zenoh.pubsub.PutOptions;
import io.zenoh.pubsub.Subscriber;
import io.zenoh.query.*;
import io.zenoh.sample.Sample;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.junit.runners.JUnit4;

import static org.junit.Assert.*;

@RunWith(JUnit4.class)
public class EncodingTest {

    private static final Encoding without_schema = Encoding.TEXT_CSV;
    private static final Encoding with_schema = Encoding.APPLICATION_JSON.withSchema("test_schema");
    private ZBytes payload = ZBytes.from("test");

    @Test
    public void encoding_subscriberTest() throws ZError, InterruptedException {
        Session session = Zenoh.open(Config.loadDefault());
        KeyExpr keyExpr = KeyExpr.tryFrom("example/testing/keyexpr");

        // Testing non null schema
        Sample[] receivedSample = new Sample[1];

        Subscriber<Void> subscriber =
                session.declareSubscriber(keyExpr, sample -> receivedSample[0] = sample);

        var putOptions = new PutOptions();
        putOptions.setEncoding(with_schema);
        session.put(keyExpr, payload, putOptions);
        Thread.sleep(200);

        assertNotNull(receivedSample[0]);
        assertEquals(receivedSample[0].getEncoding(), with_schema);

        // Testing null schema
        receivedSample[0] = null;
        putOptions.setEncoding(without_schema);
        session.put(keyExpr, payload, putOptions);
        Thread.sleep(200);

        assertEquals(receivedSample[0].getEncoding(), without_schema);

        subscriber.close();
        session.close();
    }

    @Test
    public void encoding_replySuccessTest() throws ZError, InterruptedException {
        Session session = Zenoh.open(Config.loadDefault());
        KeyExpr keyExpr = KeyExpr.tryFrom("example/testing/**");
        Selector test1 = Selector.tryFrom("example/testing/reply_success");
        Selector test2 = Selector.tryFrom("example/testing/reply_success_with_schema");

        var queryable = session.declareQueryable(keyExpr, query ->
        {
            try {
                KeyExpr queryKeyExpr = query.getKeyExpr();
                if (queryKeyExpr.equals(test1.getKeyExpr())) {
                    query.reply(queryKeyExpr, payload, new ReplyConfig().encoding(without_schema));
                } else if (queryKeyExpr.equals(test2.getKeyExpr())) {
                    query.reply(queryKeyExpr, payload, new ReplyConfig().encoding(with_schema));
                }
            } catch (Exception e) {
                throw new RuntimeException(e);
            }
        }
        );

        // Testing with null schema on a reply success scenario.
        Sample[] receivedSample = new Sample[1];
        session.get(test1, reply -> {
            assertTrue(reply instanceof Reply.Success);
            receivedSample[0] = ((Reply.Success) reply).getSample();
        });
        Thread.sleep(200);

        assertNotNull(receivedSample[0]);
        assertEquals(receivedSample[0].getEncoding(), without_schema);

        // Testing with non-null schema on a reply success scenario.
        receivedSample[0] = null;
        session.get(test2, reply -> {
            assertTrue(reply instanceof Reply.Success);
            receivedSample[0] = ((Reply.Success) reply).getSample();
        });
        Thread.sleep(200);

        assertNotNull(receivedSample[0]);
        assertEquals(receivedSample[0].getEncoding(), with_schema);

        queryable.close();
        session.close();
    }

    @Test
    public void encoding_replyErrorTest() throws ZError, InterruptedException {
        Session session = Zenoh.open(Config.loadDefault());
        KeyExpr keyExpr = KeyExpr.tryFrom("example/testing/**");
        Selector test1 = Selector.tryFrom("example/testing/reply_error");
        Selector test2 = Selector.tryFrom("example/testing/reply_error_with_schema");

        ZBytes replyPayload = ZBytes.from("test");
        var queryable = session.declareQueryable(keyExpr, query ->
        {
            KeyExpr keyExpr1 = query.getKeyExpr();
            try {
                if (keyExpr1.equals(test1.getKeyExpr())) {
                    query.replyErr(replyPayload, new ReplyErrConfig().encoding(without_schema));
                } else if (keyExpr1.equals(test2.getKeyExpr())) {
                    query.replyErr(replyPayload, new ReplyErrConfig().encoding(with_schema));
                }
            } catch (Exception e) {
                throw new RuntimeException(e);
            }
        });

        // Testing with null schema on a reply error scenario.
        ZBytes[] errorMessage = new ZBytes[1];
        Encoding[] errorEncoding = new Encoding[1];
        session.get(test1, reply ->
            {
                assertTrue(reply instanceof Reply.Error);
                Reply.Error reply1 = (Reply.Error) reply;
                errorMessage[0] = reply1.getError();
                errorEncoding[0] = reply1.getEncoding();
            }
        );
        Thread.sleep(200);

        assertNotNull(errorMessage[0]);
        assertEquals(errorEncoding[0], without_schema);

        Thread.sleep(200);

        // Testing with non-null schema on a reply error scenario.
        errorMessage[0] = null;
        errorEncoding[0] = null;
        session.get(test2, reply ->
        {
                assertTrue(reply instanceof Reply.Error);
                Reply.Error error = (Reply.Error) reply;
                errorMessage[0] = error.getError();
                errorEncoding[0] = error.getEncoding();
        });
        Thread.sleep(200);

        assertNotNull(errorMessage[0]);
        assertEquals(errorEncoding[0], with_schema);

        queryable.close();
        session.close();
    }

    @Test
    public void encoding_queryTest() throws ZError, InterruptedException {
        Session session = Zenoh.open(Config.loadDefault());
        KeyExpr keyExpr = KeyExpr.tryFrom("example/testing/keyexpr");
        Selector selector = Selector.tryFrom("example/testing/keyexpr");

        Encoding[] receivedEncoding = new Encoding[1];
        var queryable = session.declareQueryable(keyExpr, query ->
        {
            receivedEncoding[0] = query.getEncoding();
            query.close();
        });

        // Testing with null schema
        var getOptions = new GetOptions();
        getOptions.setPayload(payload);
        getOptions.setEncoding(without_schema);
        session.get(selector, getOptions);
        Thread.sleep(200);

        assertEquals(receivedEncoding[0], without_schema);

        Thread.sleep(200);

        // Testing non-null schema
        receivedEncoding[0] = null;
        getOptions.setEncoding(with_schema);
        session.get(selector, getOptions);
        Thread.sleep(200);

        assertEquals(receivedEncoding[0], with_schema);

        queryable.close();
        session.close();
    }
}
