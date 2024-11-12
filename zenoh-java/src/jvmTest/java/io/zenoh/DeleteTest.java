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
import io.zenoh.pubsub.Subscriber;
import io.zenoh.sample.SampleKind;
import io.zenoh.sample.Sample;
import kotlin.Unit;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.junit.runners.JUnit4;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;

@RunWith(JUnit4.class)
public class DeleteTest {

    @Test
    public void deleteIsProperlyReceivedBySubscriberTest() throws ZError, InterruptedException {
        Session session = Zenoh.open(Config.loadDefault());
        final Sample[] receivedSample = new Sample[1];
        KeyExpr keyExpr = KeyExpr.tryFrom("example/testing/keyexpr");
        Subscriber<Unit> subscriber = session.declareSubscriber(keyExpr).callback(sample -> receivedSample[0] = sample).res();
        session.delete(keyExpr).res();

        Thread.sleep(1000);
        subscriber.close();
        session.close();
        assertNotNull(receivedSample[0]);
        assertEquals(receivedSample[0].getKind(), SampleKind.DELETE);
    }
}
