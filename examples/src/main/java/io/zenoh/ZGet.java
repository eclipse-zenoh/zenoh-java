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

import io.zenoh.bytes.Encoding;
import io.zenoh.bytes.ZBytes;
import io.zenoh.exceptions.ZError;
import io.zenoh.query.GetConfig;
import io.zenoh.query.Selector;
import io.zenoh.query.Reply;
import io.zenoh.sample.SampleKind;
import picocli.CommandLine;

import java.time.Duration;
import java.util.List;
import java.util.Optional;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.Callable;

import static io.zenoh.ConfigKt.loadConfig;

@CommandLine.Command(
        name = "ZGet",
        mixinStandardHelpOptions = true,
        description = "Zenoh Get example"
)
public class ZGet implements Callable<Integer> {

    @Override
    public Integer call() throws ZError, InterruptedException {
        Zenoh.initLogFromEnvOr("error");
        System.out.println("Opening session...");

        Config config = loadConfig(emptyArgs, configFile, connect, listen, noMulticastScouting, mode);
        Selector selector = Selector.tryFrom(this.selectorOpt);

        // A GET query can be performed in different ways, by default (using a blocking queue), using a callback
        // or providing a handler. Uncomment one of the function calls below to try out different implementations:
        // Implementation with a blocking queue
        getExampleDefault(config, selector);
        // getExampleWithCallback(config, selector);
        // getExampleWithHandler(config, selector);
        // getExampleProvidingConfig(config, selector);

        return 0;
    }

    private void getExampleDefault(Config config, Selector selector) throws ZError, InterruptedException {
        try (Session session = Zenoh.open(config)) {
            System.out.println("Performing Get on '" + selector + "'...");
            BlockingQueue<Optional<Reply>> receiver = session.get(selector);

            while (true) {
                Optional<Reply> wrapper = receiver.take();
                if (wrapper.isEmpty()) {
                    break;
                }
                Reply reply = wrapper.get();
                handleReply(reply);
            }
        }
    }

    /**
     * Example using a simple callback for handling the replies.
     * @see io.zenoh.handlers.Callback
     */
    private void getExampleWithCallback(Config config, Selector selector) throws ZError {
        try (Session session = Zenoh.open(config)) {
            System.out.println("Performing Get on '" + selector + "'...");
            session.get(selector, this::handleReply);
        }
    }

    /**
     * Example using a custom implementation of a Handler.
     * @see QueueHandler
     * @see io.zenoh.handlers.Handler
     */
    private void getExampleWithHandler(Config config, Selector selector) throws ZError {
        try (Session session = Zenoh.open(config)) {
            System.out.println("Performing Get on '" + selector + "'...");
            QueueHandler<Reply> queueHandler = new QueueHandler<>();
            session.get(selector, queueHandler);
        }
    }

    /**
     * The purpose of this example is to show how to provide configuration parameters
     * to the get query. For this, you can optionally provide a GetConfig parameter.
     * @see GetConfig
     */
    private void getExampleProvidingConfig(Config config, Selector selector) throws ZError {
        try (Session session = Zenoh.open(config)) {
            System.out.println("Performing Get on '" + selector + "'...");

            // Build the config
            GetConfig getConfig = new GetConfig();
            getConfig.setTimeout(Duration.ofMillis(1000));
            getConfig.setEncoding(Encoding.ZENOH_STRING);
            getConfig.setPayload(ZBytes.from("Example payload"));

            // Note the syntax below is valid as well
            GetConfig getConfig2 = new GetConfig()
                    .timeout(Duration.ofMillis(1000))
                    .encoding(Encoding.ZENOH_STRING)
                    .payload(ZBytes.from("Example payload"));

            // Apply the config
            session.get(selector, this::handleReply, getConfig);
        }
    }

    private void handleReply(Reply reply) {
        if (reply instanceof Reply.Success) {
            Reply.Success successReply = (Reply.Success) reply;
            if (successReply.getSample().getKind() == SampleKind.PUT) {
                System.out.println("Received ('" + successReply.getSample().getKeyExpr() + "': '" + successReply.getSample().getPayload() + "')");
            } else if (successReply.getSample().getKind() == SampleKind.DELETE) {
                System.out.println("Received (DELETE '" + successReply.getSample().getKeyExpr() + "')");
            }
        } else {
            Reply.Error errorReply = (Reply.Error) reply;
            System.out.println("Received (ERROR: '" + errorReply.getError() + "')");
        }
    }


    /**
     * ----- Example CLI arguments and private fields -----
     */

    private final Boolean emptyArgs;

    ZGet(Boolean emptyArgs) {
        this.emptyArgs = emptyArgs;
    }

    @CommandLine.Option(
            names = {"-s", "--selector"},
            description = "The selection of resources to query [default: demo/example/**].",
            defaultValue = "demo/example/**"
    )
    private String selectorOpt;

    @CommandLine.Option(
            names = {"-p", "--payload"},
            description = "An optional payload to put in the query."
    )
    private String payload;

    @CommandLine.Option(
            names = {"-t", "--target"},
            description = "The target queryables of the query. Default: BEST_MATCHING. " +
                    "[possible values: BEST_MATCHING, ALL, ALL_COMPLETE]"
    )
    private String target;

    @CommandLine.Option(
            names = {"-o", "--timeout"},
            description = "The query timeout in milliseconds [default: 10000].",
            defaultValue = "10000"
    )
    private long timeout;

    @CommandLine.Option(
            names = {"-c", "--config"},
            description = "A configuration file."
    )
    private String configFile;

    @CommandLine.Option(
            names = {"-m", "--mode"},
            description = "The session mode. Default: peer. Possible values: [peer, client, router].",
            defaultValue = "peer"
    )
    private String mode;

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
            names = {"-a", "--attach"},
            description = "The attachment to add to the get. The key-value pairs are &-separated, and = serves as the separator between key and value."
    )
    private String attachment;

    @CommandLine.Option(
            names = {"--no-multicast-scouting"},
            description = "Disable the multicast-based scouting mechanism.",
            defaultValue = "false"
    )
    private boolean noMulticastScouting;

    public static void main(String[] args) {
        int exitCode = new CommandLine(new ZGet(args.length == 0)).execute(args);
        System.exit(exitCode);
    }
}
