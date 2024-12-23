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
import io.zenoh.liveliness.LivelinessToken;
import io.zenoh.query.Reply;
import io.zenoh.sample.Sample;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.junit.runners.JUnit4;

import static org.junit.Assert.assertNotNull;

@RunWith(JUnit4.class)
public class LivelinessTest {

    @Test
    public void getLivelinessTest() throws ZError, InterruptedException {
        Session sessionA = Zenoh.open(Config.loadDefault());
        Session sessionB = Zenoh.open(Config.loadDefault());

        var keyExpr = KeyExpr.tryFrom("test/liveliness");
        LivelinessToken token = sessionA.liveliness().declareToken(keyExpr);
        Thread.sleep(1000);

        Reply[] receivedReply = new Reply[1];
        sessionB.liveliness().get(KeyExpr.tryFrom("test/**"), reply -> receivedReply[0] = reply);

        Thread.sleep(1000);

        assertNotNull(receivedReply[0]);
        token.close();
        sessionA.close();
        sessionB.close();
    }

    @Test
    public void livelinessSubscriberTest() throws ZError, InterruptedException {
        Session sessionA = Zenoh.open(Config.loadDefault());
        Session sessionB = Zenoh.open(Config.loadDefault());

        Sample[] receivedSample = new Sample[1];

        var subscriber = sessionA.liveliness().declareSubscriber(KeyExpr.tryFrom("test/**"), sample -> receivedSample[0] = sample);
        Thread.sleep(1000);
        var token = sessionB.liveliness().declareToken(KeyExpr.tryFrom("test/liveliness"));

        Thread.sleep(1000);

        assertNotNull(receivedSample[0]);

        token.close();
        subscriber.close();
        sessionA.close();
        sessionB.close();
    }
}
