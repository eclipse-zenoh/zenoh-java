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

public class ZPubThr {

    public static void main(String[] args) throws ZError {
        int size = 8;
        byte[] data = new byte[size];
        for (int i = 0; i < size; i++) {
            data[i] = (byte) (i % 10);
        }
        try (Session session = Zenoh.open(Config.loadDefault())) {
            try (KeyExpr keyExpr = KeyExpr.tryFrom("test/thr")) {
                try (Publisher publisher = session.declarePublisher(keyExpr)) {
                    System.out.println("Publisher declared on test/thr.");
                    System.out.println("Press CTRL-C to quit...");
                    while (true) {
                        publisher.put(ZBytes.from(data)).res();
                    }
                }
            }
        }
    }
}
