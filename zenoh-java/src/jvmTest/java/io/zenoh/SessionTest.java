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
import io.zenoh.exceptions.ZError;
import io.zenoh.keyexpr.KeyExpr;
import io.zenoh.pubsub.Publisher;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.junit.runners.JUnit4;

import static org.junit.Assert.*;

@RunWith(JUnit4.class)
public class SessionTest {

    private static final KeyExpr testKeyExpr;

    static {
        try {
            testKeyExpr = KeyExpr.tryFrom("example/testing/keyexpr");
        } catch (ZError e) {
            throw new RuntimeException(e);
        }
    }

    @Test
    public void sessionStartCloseTest() throws ZError {
        Session session = Zenoh.open(Config.loadDefault());
        assertFalse(session.isClosed());
        session.close();
        assertTrue(session.isClosed());
    }

    @Test
    public void sessionClose_declarationsAreUndeclaredAfterClosingSessionTest() throws ZError, InterruptedException {
        Session session = Zenoh.open(Config.loadDefault());

        Publisher publisher = session.declarePublisher(testKeyExpr);
        var subscriber = session.declareSubscriber(testKeyExpr);
        session.close();

        Thread.sleep(1000);

        assertFalse(subscriber.isValid());
        assertFalse(publisher.isValid());

        assertThrows(ZError.class, () -> publisher.put(ZBytes.from("Test")).res());
    }

    @Test
    public void sessionClose_newDeclarationsReturnNullAfterClosingSession() throws ZError {
        Session session = Zenoh.open(Config.loadDefault());
        session.close();
        assertThrows(ZError.class, () -> session.declarePublisher(testKeyExpr));
        assertThrows(ZError.class, () -> session.declareQueryable(testKeyExpr));
        assertThrows(ZError.class, () -> session.declareSubscriber(testKeyExpr));
    }
}
