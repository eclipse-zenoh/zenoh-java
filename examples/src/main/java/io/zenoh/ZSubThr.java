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
import io.zenoh.pubsub.Subscriber;
import picocli.CommandLine;

import java.util.List;
import java.util.concurrent.Callable;

import static io.zenoh.ConfigKt.loadConfig;

@CommandLine.Command(
        name = "ZSubThr",
        mixinStandardHelpOptions = true,
        description = "Zenoh Subscriber Throughput test"
)
public class ZSubThr implements Callable<Integer> {

    @Override
    public Integer call() throws Exception {
        Zenoh.initLogFromEnvOr("error");

        Config config = loadConfig(emptyArgs, configFile, connect, listen, noMulticastScouting, mode);

        System.out.println("Opening Session");
        try (Session session = Zenoh.open(config)) {
            try (KeyExpr keyExpr = KeyExpr.tryFrom("test/thr")) {
                subscriber = session.declareSubscriber(keyExpr, sample -> listener(number));
                System.out.println("Press CTRL-C to quit...");

                while (subscriber.isValid()) {
                    Thread.sleep(1000);
                }
            }
        } catch (ZError e) {
            System.err.println("Error during Zenoh operation: " + e.getMessage());
            return 1;
        }
        return 0;
    }

    private void listener(long number) {
        if (batchCount > samples) {
            closeSubscriber();
            report();
            return;
        }

        if (count == 0) {
            startTimestampNs = System.nanoTime();
            if (globalStartTimestampNs == 0) {
                globalStartTimestampNs = startTimestampNs;
            }
            count++;
            return;
        }

        if (count < number) {
            count++;
            return;
        }

        long stop = System.nanoTime();
        double elapsedTimeSecs = (double) (stop - startTimestampNs) / NANOS_TO_SEC;
        double messagesPerSec = number / elapsedTimeSecs;
        System.out.printf("%.2f msgs/sec%n", messagesPerSec);
        batchCount++;
        count = 0;
    }

    private void report() {
        long end = System.nanoTime();
        long totalMessages = batchCount * number + count;
        double elapsedTimeSecs = (double) (end - globalStartTimestampNs) / NANOS_TO_SEC;
        double averageMessagesPerSec = totalMessages / elapsedTimeSecs;

        System.out.printf("Received %d messages in %.2f seconds: averaged %.2f msgs/sec%n",
                totalMessages, elapsedTimeSecs, averageMessagesPerSec);
    }

    private void closeSubscriber() {
        if (subscriber != null && subscriber.isValid()) {
            try {
                subscriber.close();
            } catch (Exception e) {
                System.err.println("Error closing subscriber: " + e.getMessage());
            }
        }
    }

    
    /**
     * ----- Example arguments and private fields -----
     */

    private final Boolean emptyArgs;

    ZSubThr(Boolean emptyArgs) {
        this.emptyArgs = emptyArgs;
    }

    private static final long NANOS_TO_SEC = 1_000_000_000L;
    private long batchCount = 0;
    private long count = 0;
    private long startTimestampNs = 0;
    private long globalStartTimestampNs = 0;

    @CommandLine.Option(
            names = {"-s", "--samples"},
            description = "Number of throughput measurements [default: 10].",
            defaultValue = "10"
    )
    private long samples;

    @CommandLine.Option(
            names = {"-n", "--number"},
            description = "Number of messages in each throughput measurement [default: 100000].",
            defaultValue = "100000"
    )
    private long number;

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

    private Subscriber subscriber;

    public static void main(String[] args) {
        int exitCode = new CommandLine(new ZSubThr(args.length == 0)).execute(args);
        System.exit(exitCode);
    }
}
