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
import io.zenoh.pubsub.Publisher;
import io.zenoh.pubsub.PublisherConfig;
import io.zenoh.qos.CongestionControl;
import picocli.CommandLine;

import java.util.List;
import java.util.concurrent.Callable;
import java.util.concurrent.CountDownLatch;

import static io.zenoh.ConfigKt.loadConfig;

@CommandLine.Command(
        name = "ZPong",
        mixinStandardHelpOptions = true,
        description = "Zenoh ZPong example"
)
public class ZPong implements Callable<Integer> {

    @CommandLine.Option(
            names = "--no-express",
            description = "Express for sending data.",
            defaultValue = "false"
    )
    private boolean noExpress;

    @CommandLine.Option(
            names = {"-c", "--config"},
            description = "A configuration file."
    )
    private String configFile;

    @CommandLine.Option(
            names = {"-e", "--connect"},
            description = "Endpoints to connect to.",
            split = ","
    )
    private List<String> connect;

    @CommandLine.Option(
            names = {"-l", "--listen"},
            description = "Endpoints to listen on.",
            split = ","
    )
    private List<String> listen;

    @CommandLine.Option(
            names = {"-m", "--mode"},
            description = "The session mode. Default: peer. Possible values: [peer, client, router].",
            defaultValue = "peer"
    )
    private String mode;

    @CommandLine.Option(
            names = {"--no-multicast-scouting"},
            description = "Disable the multicast-based scouting mechanism.",
            defaultValue = "false"
    )
    private boolean noMulticastScouting;

    private static final CountDownLatch latch = new CountDownLatch(1);

    @Override
    public Integer call() throws Exception {
        Zenoh.initLogFromEnvOr("error");

        Config config = loadConfig(true, configFile, connect, listen, noMulticastScouting, mode);

        System.out.println("Opening session...");
        try (Session session = Zenoh.open(config)) {
            KeyExpr keyExprPing = KeyExpr.tryFrom("test/ping");
            KeyExpr keyExprPong = KeyExpr.tryFrom("test/pong");

            Publisher publisher = session.declarePublisher(
                    keyExprPong,
                    new PublisherConfig().congestionControl(CongestionControl.BLOCK).express(!noExpress)
            );

            session.declareSubscriber(keyExprPing, sample -> {
                try {
                    publisher.put(sample.getPayload());
                } catch (ZError e) {
                    System.err.println("Error responding to ping: " + e.getMessage());
                }
            });

            latch.await();
        } catch (ZError e) {
            System.err.println("Error: " + e.getMessage());
            return 1;
        }
        return 0;
    }

    public static void main(String[] args) {
        Runtime.getRuntime().addShutdownHook(new Thread(() -> {
            System.out.println("Shutting down...");
            latch.countDown();
        }));

        int exitCode = new CommandLine(new ZPong()).execute(args);
        System.exit(exitCode);
    }
}
