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
import io.zenoh.liveliness.LivelinessSubscriberOptions;
import io.zenoh.sample.Sample;
import io.zenoh.sample.SampleKind;
import picocli.CommandLine;

import java.util.List;
import java.util.Optional;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.Callable;

import static io.zenoh.ConfigKt.loadConfig;

@CommandLine.Command(
        name = "ZSubLiveliness",
        mixinStandardHelpOptions = true,
        description = "Zenoh Sub Liveliness example"
)
public class ZSubLiveliness implements Callable<Integer> {

    @Override
    public Integer call() throws Exception {
        Zenoh.initLogFromEnvOr("error");

        Config config = loadConfig(emptyArgs, configFile, connect, listen, noMulticastScouting, mode);
        KeyExpr keyExpr = KeyExpr.tryFrom(this.key);

        // Subscribing to liveliness tokens can be implemented in multiple ways.
        // Uncomment the desired implementation:
        subscribeToLivelinessWithBlockingQueue(config, keyExpr);
        // subscribeToLivelinessWithCallback(config, keyExpr);
        // subscribeToLivelinessWithHandler(config, keyExpr);

        return 0;
    }

    /**
     * Default implementation using a blocking queue to handle incoming liveliness tokens.
     */
    private void subscribeToLivelinessWithBlockingQueue(Config config, KeyExpr keyExpr) throws ZError, InterruptedException {
        try (Session session = Zenoh.open(config)) {
            var options = new LivelinessSubscriberOptions(history);
            var subscriber = session.liveliness().declareSubscriber(keyExpr, options);

            BlockingQueue<Optional<Sample>> receiver = subscriber.getReceiver();
            System.out.println("Listening for liveliness tokens...");
            while (true) {
                Optional<Sample> wrapper = receiver.take();
                if (wrapper.isEmpty()) {
                    break;
                }
                handleLivelinessSample(wrapper.get());
            }
        }
    }

    /**
     * Example using a callback to handle incoming liveliness tokens asynchronously.
     *
     * @see io.zenoh.handlers.Callback
     */
    private void subscribeToLivelinessWithCallback(Config config, KeyExpr keyExpr) throws ZError {
        try (Session session = Zenoh.open(config)) {
            var options = new LivelinessSubscriberOptions(history);
            session.liveliness().declareSubscriber(
                    keyExpr,
                    this::handleLivelinessSample,
                    options
            );
        }
    }

    /**
     * Example using a handler to handle incoming liveliness tokens asynchronously.
     *
     * @see io.zenoh.handlers.Handler
     * @see QueueHandler
     */
    private void subscribeToLivelinessWithHandler(Config config, KeyExpr keyExpr) throws ZError {
        try (Session session = Zenoh.open(config)) {
            QueueHandler<Sample> queueHandler = new QueueHandler<>();
            var options = new LivelinessSubscriberOptions(history);
            session.liveliness().declareSubscriber(
                    keyExpr,
                    queueHandler,
                    options
            );
        }
    }

    /**
     * Handles a single liveliness token sample.
     */
    private void handleLivelinessSample(Sample sample) {
        if (sample.getKind() == SampleKind.PUT) {
            System.out.println(">> [LivelinessSubscriber] New alive token ('" + sample.getKeyExpr() + "')");
        } else if (sample.getKind() == SampleKind.DELETE) {
            System.out.println(">> [LivelinessSubscriber] Dropped token ('" + sample.getKeyExpr() + "')");
        }
    }

    /**
     * ----- Example arguments and private fields -----
     */

    private final Boolean emptyArgs;

    ZSubLiveliness(Boolean emptyArgs) {
        this.emptyArgs = emptyArgs;
    }

    @CommandLine.Option(
            names = {"-c", "--config"},
            description = "A configuration file."
    )
    private String configFile;

    @CommandLine.Option(
            names = {"-k", "--key"},
            description = "The key expression to subscribe to [default: group1/**].",
            defaultValue = "group1/**"
    )
    private String key;

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
            names = {"--history"},
            description = "Get historical liveliness tokens.",
            defaultValue = "false"
    )
    private boolean history;

    @CommandLine.Option(
            names = {"--no-multicast-scouting"},
            description = "Disable the multicast-based scouting mechanism.",
            defaultValue = "false"
    )
    private boolean noMulticastScouting;

    public static void main(String[] args) {
        int exitCode = new CommandLine(new ZSubLiveliness(args.length == 0)).execute(args);
        System.exit(exitCode);
    }
}
