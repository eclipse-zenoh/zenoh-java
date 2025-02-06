//
// Copyright (c) 2023 ZettaScale Technology
//
// This program and the accompanying materials are made available under the
// terms of the Eclipse Public License 2.0 which is available at
// http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0.
//
// SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
//
// Contributors:
//   ZettaScale Zenoh Team, <zenoh@zettascale.tech>
//

package io.zenoh;

import io.zenoh.exceptions.ZError;
import io.zenoh.keyexpr.KeyExpr;
import io.zenoh.query.Reply;
import picocli.CommandLine;

import java.time.Duration;
import java.util.List;
import java.util.Optional;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.Callable;

import static io.zenoh.ConfigKt.loadConfig;

@CommandLine.Command(
        name = "ZGetLiveliness",
        mixinStandardHelpOptions = true,
        description = "Zenoh Get Liveliness example"
)
public class ZGetLiveliness implements Callable<Integer> {

    @Override
    public Integer call() throws Exception {
        Zenoh.initLogFromEnvOr("error");

        Config config = loadConfig(emptyArgs, configFile, connect, listen, noMulticastScouting, mode);
        KeyExpr keyExpr = KeyExpr.tryFrom(this.key);

        Session session = Zenoh.open(config);

        // Uncomment one of the lines below to try out different implementations:
        getLivelinessWithBlockingQueue(session, keyExpr);
        // getLivelinessWithCallback(session, keyExpr);
        // getLivelinessWithHandler(session, keyExpr);

        return 0;
    }

    /**
     * Default implementation using a blocking queue to handle replies.
     */
    private void getLivelinessWithBlockingQueue(Session session, KeyExpr keyExpr) throws ZError, InterruptedException {
        System.out.println("Sending Liveliness Query '" + keyExpr + "'.");
        BlockingQueue<Optional<Reply>> replyQueue = session.liveliness().get(keyExpr, Duration.ofMillis(timeout));

        while (true) {
            Optional<Reply> wrapper = replyQueue.take();
            if (wrapper.isEmpty()) {
                break;
            }
            handleReply(wrapper.get());
        }
    }

    /**
     * Example using a callback to handle liveliness replies asynchronously.
     * @see io.zenoh.handlers.Callback
     */
    private void getLivelinessWithCallback(Session session, KeyExpr keyExpr) throws ZError {
        System.out.println("Sending Liveliness Query '" + keyExpr + "'.");
        session.liveliness().get(keyExpr, this::handleReply, Duration.ofMillis(timeout));
    }

    /**
     * Example using a custom handler to process liveliness replies.
     * @see QueueHandler
     * @see io.zenoh.handlers.Handler
     */
    private void getLivelinessWithHandler(Session session, KeyExpr keyExpr) throws ZError {
        System.out.println("Sending Liveliness Query '" + keyExpr + "'.");
        QueueHandler<Reply> queueHandler = new QueueHandler<>();
        session.liveliness().get(keyExpr, queueHandler, Duration.ofMillis(timeout));
    }

    private void handleReply(Reply reply) {
        if (reply instanceof Reply.Success) {
            Reply.Success successReply = (Reply.Success) reply;
            System.out.println(">> Alive token ('" + successReply.getSample().getKeyExpr() + "')");
        } else if (reply instanceof Reply.Error) {
            Reply.Error errorReply = (Reply.Error) reply;
            System.out.println(">> Received (ERROR: '" + errorReply.getError() + "')");
        }
    }

    /**
     * ----- Example arguments and private fields -----
     */

    private final Boolean emptyArgs;

    ZGetLiveliness(Boolean emptyArgs) {
        this.emptyArgs = emptyArgs;
    }

    @CommandLine.Option(
            names = {"-c", "--config"},
            description = "A configuration file."
    )
    private String configFile;

    @CommandLine.Option(
            names = {"-k", "--key"},
            description = "The key expression matching liveliness tokens to query. [default: group1/**].",
            defaultValue = "group1/**"
    )
    private String key;

    @CommandLine.Option(
            names = {"-o", "--timeout"},
            description = "The query timeout in milliseconds [default: 10000].",
            defaultValue = "10000"
    )
    private long timeout;

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
        int exitCode = new CommandLine(new ZGetLiveliness(args.length == 0)).execute(args);
        System.exit(exitCode);
    }
}
