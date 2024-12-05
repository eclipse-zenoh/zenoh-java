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
import io.zenoh.scouting.Hello;
import io.zenoh.scouting.Scout;
import io.zenoh.scouting.ScoutOptions;
import picocli.CommandLine;

import java.util.Optional;
import java.util.Set;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.Callable;

@CommandLine.Command(
        name = "ZScout",
        mixinStandardHelpOptions = true,
        description = "Zenoh Scouting example"
)
public class ZScout implements Callable<Integer> {

    @Override
    public Integer call() throws Exception {
        Zenoh.initLogFromEnvOr("error");

        System.out.println("Scouting...");

        var scoutOptions = new ScoutOptions();
        scoutOptions.setWhatAmI(Set.of(WhatAmI.Peer, WhatAmI.Router));
        Scout<BlockingQueue<Optional<Hello>>> scout = Zenoh.scout(scoutOptions);
        BlockingQueue<Optional<Hello>> receiver = scout.getReceiver();
        assert receiver != null;

        try {
            while (true) {
                Optional<Hello> wrapper = receiver.take();
                if (wrapper.isEmpty()) {
                    break;
                }

                Hello hello = wrapper.get();
                System.out.println(hello);
            }
        } finally {
            scout.stop();
        }

        return 0;
    }

    public static void main(String[] args) {
        int exitCode = new CommandLine(new ZScout()).execute(args);
        System.exit(exitCode);
    }
}
