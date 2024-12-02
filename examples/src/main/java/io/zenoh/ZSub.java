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
import io.zenoh.handlers.Handler;
import io.zenoh.keyexpr.KeyExpr;
import io.zenoh.pubsub.Subscriber;
import io.zenoh.pubsub.SubscriberConfig;
import io.zenoh.sample.Sample;
import picocli.CommandLine;

import java.util.List;
import java.util.Optional;
import java.util.Queue;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.Callable;

import static io.zenoh.ConfigKt.loadConfig;

@CommandLine.Command(
        name = "ZSub",
        mixinStandardHelpOptions = true,
        description = "Zenoh Sub example"
)
public class ZSub implements Callable<Integer> {

    @Override
    public Integer call() throws Exception {
        Zenoh.initLogFromEnvOr("error");

        Config config = loadConfig(emptyArgs, configFile, connect, listen, noMulticastScouting, mode);
        KeyExpr keyExpr = KeyExpr.tryFrom(this.key);

        // Subscribers can be declared in different ways.
        // Uncomment one of the lines below to try out different implementations:
        subscribeWithBlockingQueue(config, keyExpr);
        // subscribeWithCallback(config, keyExpr);
        // subscribeWithHandler(config, keyExpr);

        return 0;
    }

    /**
     * Default implementation using a blocking queue to handle incoming samples.
     */
    private void subscribeWithBlockingQueue(Config config, KeyExpr keyExpr) throws ZError, InterruptedException {
        try (Session session = Zenoh.open(config)) {
            try (Subscriber<BlockingQueue<Optional<Sample>>> subscriber = session.declareSubscriber(keyExpr)) {
                BlockingQueue<Optional<Sample>> receiver = subscriber.getReceiver();
                assert receiver != null;
                while (true) {
                    Optional<Sample> wrapper = receiver.take();
                    if (wrapper.isEmpty()) {
                        break;
                    }
                    handleSample(wrapper.get());
                }
            }
        }
    }

    /**
     * Example using a callback to handle incoming samples asynchronously.
     * @see io.zenoh.handlers.Callback
     */
    private void subscribeWithCallback(Config config, KeyExpr keyExpr) throws ZError {
        try (Session session = Zenoh.open(config)) {
            session.declareSubscriber(keyExpr, this::handleSample);
        }
    }

    /**
     * Example using a custom implementation of the Handler.
     * @see QueueHandler
     * @see Handler
     */
    private void subscribeWithHandler(Config config, KeyExpr keyExpr) throws ZError {
        try (Session session = Zenoh.open(config)) {
            QueueHandler<Sample> queueHandler = new QueueHandler<>();
            session.declareSubscriber(keyExpr, queueHandler);
        }
    }

    /**
     * Handles a single Sample and prints relevant information.
     */
    private void handleSample(Sample sample) {
        String attachment = sample.getAttachment() != null ? ", with attachment: " + sample.getAttachment() : "";
        System.out.println(">> [Subscriber] Received " + sample.getKind() +
                " ('" + sample.getKeyExpr() + "': '" + sample.getPayload() + "'" + attachment + ")");
    }

    /**
     * ----- Example arguments and private fields -----
     */

    private final Boolean emptyArgs;

    ZSub(Boolean emptyArgs) {
        this.emptyArgs = emptyArgs;
    }

    @CommandLine.Option(
            names = {"-c", "--config"},
            description = "A configuration file."
    )
    private String configFile;

    @CommandLine.Option(
            names = {"-k", "--key"},
            description = "The key expression to subscribe to [default: demo/example/**].",
            defaultValue = "demo/example/**"
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
            names = {"--no-multicast-scouting"},
            description = "Disable the multicast-based scouting mechanism.",
            defaultValue = "false"
    )
    private boolean noMulticastScouting;

    public static void main(String[] args) {
        int exitCode = new CommandLine(new ZSub(args.length == 0)).execute(args);
        System.exit(exitCode);
    }
}
