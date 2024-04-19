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

import io.zenoh.exceptions.ZenohException;
import io.zenoh.keyexpr.KeyExpr;
import io.zenoh.subscriber.Subscriber;
import kotlin.Unit;

public class ZSubThr {

    private static final long NANOS_TO_SEC = 1_000_000_000L;
    private static final long n = 50000L;
    private static int count = 0;
    private static long startTimestampNs = 0;

    public static void listener() {
        if (count == 0) {
            startTimestampNs = System.nanoTime();
            count++;
            return;
        }
        if (count < n) {
            count++;
            return;
        }
        long stop = System.nanoTime();
        double msgs = (double) (n * NANOS_TO_SEC) / (stop - startTimestampNs);
        System.out.println(msgs + " msgs/sec");
        count = 0;
    }

    public static void main(String[] args) throws ZenohException, InterruptedException {
        System.out.println("Opening Session");
        try (Session session = Session.open()) {
            try (KeyExpr keyExpr = KeyExpr.tryFrom("test/thr")) {
                try (Subscriber<Unit> subscriber = session.declareSubscriber(keyExpr).with(sample -> listener()).res()) {
                    System.out.println("Press CTRL-C to quit...");
                    while (true) {
                        Thread.sleep(1000);
                    }
                }
            }
        }
    }
}
