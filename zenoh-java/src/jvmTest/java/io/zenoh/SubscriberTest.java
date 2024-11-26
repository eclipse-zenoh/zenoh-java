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
import io.zenoh.handlers.Handler;
import io.zenoh.keyexpr.KeyExpr;
import io.zenoh.pubsub.PutConfig;
import io.zenoh.qos.CongestionControl;
import io.zenoh.qos.Priority;
import io.zenoh.sample.Sample;
import kotlin.Pair;
import org.junit.After;
import org.junit.Before;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.junit.runners.JUnit4;

import java.util.ArrayDeque;
import java.util.ArrayList;

import static org.junit.Assert.assertEquals;

@RunWith(JUnit4.class)
public class SubscriberTest {

    private static final Priority TEST_PRIORITY = Priority.DATA_HIGH;
    private static final CongestionControl TEST_CONGESTION_CONTROL = CongestionControl.BLOCK;
    private static final ArrayList<Pair<ZBytes, Encoding>> TEST_VALUES = new ArrayList<>();
    private static final KeyExpr testKeyExpr;

    static {
        TEST_VALUES.add(new Pair<>(ZBytes.from("Test 1"), Encoding.TEXT_PLAIN));
        TEST_VALUES.add(new Pair<>(ZBytes.from("Test 2"), Encoding.TEXT_JSON));
        TEST_VALUES.add(new Pair<>(ZBytes.from("Test 3"), Encoding.TEXT_CSV));
        try {
            testKeyExpr = KeyExpr.tryFrom("example/testing/keyexpr");
        } catch (ZError e) {
            throw new RuntimeException(e);
        }
    }

    private Session session = null;

    @Before
    public void setUp() throws ZError {
        session = Zenoh.open(Config.loadDefault());
    }

    @After
    public void tearDown() {
        session.close();
    }

    @Test
    public void subscriber_runsWithCallback() throws ZError {
        var receivedSamples = new ArrayList<Sample>();

        var subscriber =
                session.declareSubscriber(testKeyExpr, receivedSamples::add);

        TEST_VALUES.forEach(value -> {
                    try {
                        session.put(testKeyExpr, value.getFirst(), new PutConfig()
                                .encoding(value.getSecond())
                                .priority(TEST_PRIORITY)
                                .congestionControl(TEST_CONGESTION_CONTROL));
                    } catch (ZError e) {
                        throw new RuntimeException(e);
                    }
                }
        );
        assertEquals(receivedSamples.size(), TEST_VALUES.size());

        for (int i = 0; i < TEST_VALUES.size(); i++) {
            var valueSent = TEST_VALUES.get(i);
            var valueRecv = receivedSamples.get(i);

            assertEquals(valueRecv.getPayload(), valueSent.getFirst());
            assertEquals(valueRecv.getEncoding(), valueSent.getSecond());
            assertEquals(valueRecv.getPriority(), TEST_PRIORITY);
            assertEquals(valueRecv.getCongestionControl(), TEST_CONGESTION_CONTROL);
        }

        subscriber.close();
    }

    @Test
    public void subscriber_runsWithHandler() throws ZError {
        var handler = new QueueHandler<Sample>();
        var subscriber =
                session.declareSubscriber(testKeyExpr, handler);

        TEST_VALUES.forEach(value -> {
                try {
                    session.put(testKeyExpr, value.getFirst(), new PutConfig()
                            .encoding(value.getSecond())
                            .priority(TEST_PRIORITY)
                            .congestionControl(TEST_CONGESTION_CONTROL));
                } catch (ZError e) {
                    throw new RuntimeException(e);
                }
            }
        );
        assertEquals(handler.queue.size(), TEST_VALUES.size());

        for (int i = 0; i < TEST_VALUES.size(); i++) {
            var valueSent = TEST_VALUES.get(i);
            var valueRecv = handler.queue.poll();

            assert valueRecv != null;
            assertEquals(valueRecv.getPayload(), valueSent.getFirst());
            assertEquals(valueRecv.getEncoding(), valueSent.getSecond());
            assertEquals(valueRecv.getPriority(), TEST_PRIORITY);
            assertEquals(valueRecv.getCongestionControl(), TEST_CONGESTION_CONTROL);
        }

        subscriber.close();
    }
}

class QueueHandler<T extends ZenohType> implements Handler<T, ArrayDeque<T>> {

    final ArrayDeque<T> queue = new ArrayDeque<>();

    @Override
    public void handle(T t) {
        queue.add(t);
    }

    @Override
    public ArrayDeque<T> receiver() {
        return queue;
    }

    @Override
    public void onClose() {}
}
