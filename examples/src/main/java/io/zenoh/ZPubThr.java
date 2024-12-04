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
import io.zenoh.keyexpr.KeyExpr;
import io.zenoh.pubsub.Publisher;
import io.zenoh.pubsub.PublisherOptions;
import io.zenoh.qos.CongestionControl;
import io.zenoh.qos.Priority;
import picocli.CommandLine;

import java.util.List;
import java.util.concurrent.Callable;

import static io.zenoh.ConfigKt.loadConfig;

@CommandLine.Command(
        name = "ZPubThr",
        mixinStandardHelpOptions = true,
        description = "Zenoh Throughput example"
)
public class ZPubThr implements Callable<Integer> {

    @Override
    public Integer call() throws Exception {
        Zenoh.initLogFromEnvOr("error");

        byte[] data = new byte[payloadSize];
        for (int i = 0; i < payloadSize; i++) {
            data[i] = (byte) (i % 10);
        }
        ZBytes payload = ZBytes.from(data);

        Config config = loadConfig(emptyArgs, configFile, connect, listen, noMulticastScouting, mode);

        try (Session session = Zenoh.open(config)) {
            KeyExpr keyExpr = KeyExpr.tryFrom("test/thr");
            var publisherConfig = new PublisherOptions()
                    .congestionControl(CongestionControl.BLOCK)
                    .priority(priorityInput != null ? Priority.getEntries().get(priorityInput) : Priority.DATA);
            try (Publisher publisher = session.declarePublisher(keyExpr, publisherConfig)) {
                System.out.println("Publisher declared on test/thr.");
                long count = 0;
                long start = System.currentTimeMillis();
                System.out.println("Press CTRL-C to quit...");

                while (true) {
                    publisher.put(payload);

                    if (statsPrint) {
                        if (count < number) {
                            count++;
                        } else {
                            long elapsedTime = System.currentTimeMillis() - start;
                            long throughput = (count * 1000) / elapsedTime;
                            System.out.println(throughput + " msgs/s");
                            count = 0;
                            start = System.currentTimeMillis();
                        }
                    }
                }
            }
        }
    }


    /**
     * ----- Example CLI arguments and private fields -----
     */

    private final Boolean emptyArgs;

    ZPubThr(Boolean emptyArgs) {
        this.emptyArgs = emptyArgs;
    }

    @CommandLine.Parameters(
            index = "0",
            description = "Sets the size of the payload to publish [default: 8].",
            defaultValue = "8"
    )
    private int payloadSize;

    @CommandLine.Option(
            names = {"-p", "--priority"},
            description = "Priority for sending data."
    )
    private Integer priorityInput;

    @CommandLine.Option(
            names = {"-n", "--number"},
            description = "Number of messages in each throughput measurement [default: 100000].",
            defaultValue = "100000"
    )
    private long number;

    @CommandLine.Option(
            names = {"-t", "--print"},
            description = "Print the statistics.",
            defaultValue = "true"
    )
    private boolean statsPrint;

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
        int exitCode = new CommandLine(new ZPubThr(args.length == 0)).execute(args);
        System.exit(exitCode);
    }
}
