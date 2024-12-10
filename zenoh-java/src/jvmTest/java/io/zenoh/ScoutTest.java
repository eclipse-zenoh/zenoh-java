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

import io.zenoh.config.WhatAmI;
import io.zenoh.exceptions.ZError;
import io.zenoh.scouting.Hello;
import io.zenoh.scouting.Scout;
import io.zenoh.scouting.ScoutOptions;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.junit.runners.JUnit4;

import java.util.ArrayList;
import java.util.Optional;
import java.util.Set;
import java.util.concurrent.BlockingQueue;

import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertTrue;

@RunWith(JUnit4.class)
public class ScoutTest {

    @Test
    public void scouting_queueTest() throws ZError, InterruptedException {
        Session session = Zenoh.open(Config.loadDefault());

        Thread.sleep(1000);

        var scout = Zenoh.scout();

        Thread.sleep(1000);
        scout.close();

        ArrayList<Optional<Hello>> helloList = new ArrayList<>();
        scout.getReceiver().drainTo(helloList);

        assertTrue(helloList.size() > 1);
        for (int i = 0; i < helloList.size() - 1; i++) {
            assertTrue(helloList.get(i).isPresent());
        }
        assertTrue(helloList.get(helloList.size() - 1).isEmpty());
        session.close();
    }

    @Test
    public void scouting_callbackTest() throws ZError, InterruptedException {
        Session session = Zenoh.open(Config.loadDefault());

        Hello[] hello = new Hello[1];
        Zenoh.scout(hello1 -> hello[0] = hello1);

        Thread.sleep(1000);

        assertNotNull(hello[0]);
        session.close();
    }

    @Test
    public void scouting_whatAmITest() throws ZError {
        var scoutOptions = new ScoutOptions();
        scoutOptions.setWhatAmI(Set.of(WhatAmI.Client, WhatAmI.Peer));
        var scout = Zenoh.scout(scoutOptions);
        scout.close();
    }

    @Test
    public void scouting_onCloseTest() throws ZError {
        var scout = Zenoh.scout();
        var receiver = scout.getReceiver();

        scout.close();
        var element = receiver.poll();
        assertNotNull(element);
        assertTrue(element.isEmpty());
    }
}
