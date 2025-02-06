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
import io.zenoh.pubsub.PublisherOptions;
import io.zenoh.qos.CongestionControl;
import io.zenoh.sample.Sample;
import picocli.CommandLine;

import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.Callable;

import static io.zenoh.ConfigKt.loadConfig;

@CommandLine.Command(
        name = "ZPing",
        mixinStandardHelpOptions = true,
        description = "Zenoh Ping example"
)
public class ZPing implements Callable<Integer> {

    @Override
    public Integer call() throws Exception {
        Zenoh.initLogFromEnvOr("error");

        Config config = loadConfig(true, configFile, connect, listen, noMulticastScouting, mode);

        System.out.println("Opening session...");
        try (Session session = Zenoh.open(config)) {
            KeyExpr keyExprPing = KeyExpr.tryFrom("test/ping");
            KeyExpr keyExprPong = KeyExpr.tryFrom("test/pong");

            BlockingQueue<Optional<Sample>> receiverQueue =
                    session.declareSubscriber(keyExprPong).getReceiver();

            var publisherOptions = new PublisherOptions();
            publisherOptions.setCongestionControl(CongestionControl.BLOCK);
            publisherOptions.setExpress(!noExpress);
            Publisher publisher = session.declarePublisher(keyExprPing, publisherOptions);

            byte[] data = new byte[payloadSize];
            for (int i = 0; i < payloadSize; i++) {
                data[i] = (byte) (i % 10);
            }
            ZBytes payload = ZBytes.from(data);

            // Warm-up
            System.out.println("Warming up for " + warmup + " seconds...");
            long warmupEnd = System.currentTimeMillis() + (long) (warmup * 1000);
            while (System.currentTimeMillis() < warmupEnd) {
                publisher.put(payload);
                receiverQueue.take();
            }

            List<Long> samples = new ArrayList<>();
            for (int i = 0; i < n; i++) {
                long startTime = System.nanoTime();
                publisher.put(payload);
                receiverQueue.take();
                long elapsedTime = (System.nanoTime() - startTime) / 1000; // Convert to microseconds
                samples.add(elapsedTime);
            }

            for (int i = 0; i < samples.size(); i++) {
                long rtt = samples.get(i);
                System.out.printf("%d bytes: seq=%d rtt=%dµs lat=%dµs%n", payloadSize, i, rtt, rtt / 2);
            }
        }
        return 0;
    }


    /**
     * ----- Example CLI arguments and private fields -----
     */

    @CommandLine.Parameters(
            paramLabel = "payload_size",
            description = "Sets the size of the payload to publish [default: 8].",
            defaultValue = "8"
    )
    private int payloadSize;

    @CommandLine.Option(
            names = "--no-express",
            description = "Express for sending data.",
            defaultValue = "false"
    )
    private boolean noExpress;

    @CommandLine.Option(
            names = {"-w", "--warmup"},
            description = "The number of seconds to warm up [default: 1.0].",
            defaultValue = "1.0"
    )
    private double warmup;

    @CommandLine.Option(
            names = {"-n", "--samples"},
            description = "The number of round-trips to measure [default: 100].",
            defaultValue = "100"
    )
    private int n;

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

    public static void main(String[] args) {
        int exitCode = new CommandLine(new ZPing()).execute(args);
        System.exit(exitCode);
    }
}
