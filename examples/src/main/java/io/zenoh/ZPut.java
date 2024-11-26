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
import io.zenoh.pubsub.PutConfig;
import io.zenoh.qos.CongestionControl;
import io.zenoh.qos.Priority;

public class ZPut {
    public static void main(String[] args) throws ZError {
        System.out.println("Opening session...");
        try (Session session = Zenoh.open(Config.loadDefault())) {
            try (KeyExpr keyExpr = KeyExpr.tryFrom("demo/example/zenoh-java-put")) {
                String value = "Put from Java!";
                session.put(keyExpr, ZBytes.from(value), new PutConfig().congestionControl(CongestionControl.BLOCK).priority(Priority.REALTIME));
                System.out.println("Putting Data ('" + keyExpr + "': '" + value + "')...");
            }
        }
    }
}
