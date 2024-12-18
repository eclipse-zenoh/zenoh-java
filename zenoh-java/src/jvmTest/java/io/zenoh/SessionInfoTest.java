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

import io.zenoh.config.ZenohId;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.junit.runners.JUnit4;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;

@RunWith(JUnit4.class)
public class SessionInfoTest {

    @Test
    public void peersZidTest() throws Exception {
        String jsonConfig = "{\n" +
                "    mode: \"peer\",\n" +
                "    connect: {\n" +
                "        endpoints: [\"tcp/localhost:7450\"]\n" +
                "    }\n" +
                "}";

        Config listenConfig = Config.fromJson("{\n" +
                "    mode: \"peer\",\n" +
                "    listen: {\n" +
                "        endpoints: [\"tcp/localhost:7450\"]\n" +
                "    }\n" +
                "}");

        Session sessionC = Zenoh.open(listenConfig);
        Session sessionA = Zenoh.open(Config.fromJson(jsonConfig));
        Session sessionB = Zenoh.open(Config.fromJson(jsonConfig));

        ZenohId idA = sessionA.info().zid();
        ZenohId idB = sessionB.info().zid();
        var peers = sessionC.info().peersZid();
        assertTrue(peers.contains(idA));
        assertTrue(peers.contains(idB));

        sessionA.close();
        sessionB.close();
        sessionC.close();
    }

    @Test
    public void routersZidTest() throws Exception {
        Session session = Zenoh.open(Config.fromJson("{\n" +
                "    mode: \"router\",\n" +
                "    listen: {\n" +
                "        endpoints: [\"tcp/localhost:7450\"]\n" +
                "    }\n" +
                "}"));

        Session connectedRouterA = Zenoh.open(Config.fromJson("{\n" +
                "    mode: \"router\",\n" +
                "    connect: {\n" +
                "        endpoints: [\"tcp/localhost:7450\"]\n" +
                "    },\n" +
                "    listen: {\n" +
                "        endpoints: [\"tcp/localhost:7451\"]\n" +
                "    }\n" +
                "}"));

        Session connectedRouterB = Zenoh.open(Config.fromJson("{\n" +
                "    mode: \"router\",\n" +
                "    connect: {\n" +
                "        endpoints: [\"tcp/localhost:7450\"]\n" +
                "    },\n" +
                "    listen: {\n" +
                "        endpoints: [\"tcp/localhost:7452\"]\n" +
                "    }\n" +
                "}"));

        ZenohId idA = connectedRouterA.info().zid();
        ZenohId idB = connectedRouterB.info().zid();

        var routers = session.info().routersZid();

        assertTrue(routers.contains(idA));
        assertTrue(routers.contains(idB));

        connectedRouterA.close();
        connectedRouterB.close();
        session.close();
    }

    @Test
    public void zidTest() throws Exception {
        String jsonConfig = "{\n" +
                "    id: \"123456\"\n" +
                "}";

        Session session = Zenoh.open(Config.fromJson(jsonConfig));
        assertEquals("123456", session.info().zid().toString());
        session.close();
    }
}
