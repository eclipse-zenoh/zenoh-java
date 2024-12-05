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
import io.zenoh.query.Query;
import io.zenoh.query.Queryable;
import io.zenoh.query.QueryableOptions;
import io.zenoh.query.ReplyOptions;
import org.apache.commons.net.ntp.TimeStamp;
import picocli.CommandLine;

import java.util.List;
import java.util.Optional;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.Callable;

import static io.zenoh.ConfigKt.loadConfig;

@CommandLine.Command(
        name = "ZQueryable",
        mixinStandardHelpOptions = true,
        description = "Zenoh Queryable example"
)
public class ZQueryable implements Callable<Integer> {

    @Override
    public Integer call() throws Exception {
        Zenoh.initLogFromEnvOr("error");

        Config config = loadConfig(emptyArgs, configFile, connect, listen, noMulticastScouting, mode);
        KeyExpr keyExpr = KeyExpr.tryFrom(this.key);

        // A Queryable can be implemented in multiple ways. Uncomment one to try:
        declareQueryableWithBlockingQueue(config, keyExpr);
        // declareQueryableWithCallback(config, keyExpr);
        // declareQueryableProvidingConfig(config, keyExpr);

        return 0;
    }

    /**
     * Default implementation using a blocking queue to handle incoming queries.
     */
    private void declareQueryableWithBlockingQueue(Config config, KeyExpr keyExpr) throws ZError, InterruptedException {
        try (Session session = Zenoh.open(config)) {
            Queryable<BlockingQueue<Optional<Query>>> queryable = session.declareQueryable(keyExpr);
            BlockingQueue<Optional<Query>> receiver = queryable.getReceiver();
            assert receiver != null;
            while (true) {
                Optional<Query> wrapper = receiver.take();
                if (wrapper.isEmpty()) {
                    break;
                }
                Query query = wrapper.get();
                handleQuery(query);
            }
        }
    }

    /**
     * Example using a callback to handle incoming queries asynchronously.
     *
     * @see io.zenoh.handlers.Callback
     */
    private void declareQueryableWithCallback(Config config, KeyExpr keyExpr) throws ZError {
        try (Session session = Zenoh.open(config)) {
            session.declareQueryable(keyExpr, this::handleQuery);
        }
    }

    /**
     * Example demonstrating the use of QueryableConfig to declare a Queryable.
     *
     * @see QueryableOptions
     */
    private void declareQueryableProvidingConfig(Config config, KeyExpr keyExpr) throws ZError {
        try (Session session = Zenoh.open(config)) {
            QueryableOptions queryableOptions = new QueryableOptions();
            queryableOptions.setComplete(true);
            session.declareQueryable(keyExpr, this::handleQuery, queryableOptions);
        }
    }

    private void handleQuery(Query query) {
        try {
            String valueInfo = query.getPayload() != null ? " with value '" + query.getPayload() + "'" : "";
            System.out.println(">> [Queryable] Received Query '" + query.getSelector() + "'" + valueInfo);
            var options = new ReplyOptions();
            options.setTimeStamp(TimeStamp.getCurrentTime());
            query.reply(query.getKeyExpr(), ZBytes.from(value), options);
        } catch (Exception e) {
            System.err.println(">> [Queryable] Error sending reply: " + e.getMessage());
        }
    }

    /**
     * ----- Example arguments and private fields -----
     */

    private final Boolean emptyArgs;

    ZQueryable(Boolean emptyArgs) {
        this.emptyArgs = emptyArgs;
    }

    @CommandLine.Option(
            names = {"-k", "--key"},
            description = "The key expression to write to [default: demo/example/zenoh-java-queryable].",
            defaultValue = "demo/example/zenoh-java-queryable"
    )
    private String key;

    @CommandLine.Option(
            names = {"-v", "--value"},
            description = "The value to reply to queries [default: 'Queryable from Java!'].",
            defaultValue = "Queryable from Java!"
    )
    private String value;

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
            names = {"--no-multicast-scouting"},
            description = "Disable the multicast-based scouting mechanism.",
            defaultValue = "false"
    )
    private boolean noMulticastScouting;

    public static void main(String[] args) {
        int exitCode = new CommandLine(new ZQueryable(args.length == 0)).execute(args);
        System.exit(exitCode);
    }
}
