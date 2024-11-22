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

import io.zenoh.bytes.ZBytes;
import io.zenoh.bytes.Encoding;
import io.zenoh.exceptions.ZError;
import io.zenoh.keyexpr.KeyExpr;
import io.zenoh.pubsub.Subscriber;
import io.zenoh.pubsub.SubscriberConfig;
import io.zenoh.sample.Sample;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.junit.runners.JUnit4;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;

@RunWith(JUnit4.class)
public class PutTest {

    public static final String TEST_KEY_EXP = "example/testing/keyexpr";
    public static final ZBytes TEST_PAYLOAD = ZBytes.from("Hello");

    @Test
    public void putTest() throws ZError {
        Session session = Zenoh.open(Config.loadDefault());
        Sample[] receivedSample = new Sample[1];
        var keyExpr = KeyExpr.tryFrom(TEST_KEY_EXP);

        SubscriberConfig<Void> config = new SubscriberConfig<>();
        config.setCallback(sample -> receivedSample[0] = sample);
        Subscriber<Void> subscriber =
                session.declareSubscriber(keyExpr, config);

        session.put(keyExpr, TEST_PAYLOAD).encoding(Encoding.TEXT_PLAIN).res();
        subscriber.close();
        session.close();
        assertNotNull(receivedSample[0]);
        assertEquals(TEST_PAYLOAD, receivedSample[0].getPayload());
    }
}
