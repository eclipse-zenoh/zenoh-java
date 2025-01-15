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
import io.zenoh.query.*;
import picocli.CommandLine;

import java.time.Duration;
import java.util.List;
import java.util.concurrent.Callable;

import static io.zenoh.ConfigKt.loadConfig;

@CommandLine.Command(
        name = "ZQuerier",
        mixinStandardHelpOptions = true,
        description = "Zenoh Querier example"
)
public class ZQuerier implements Callable<Integer> {

    @Override
    public Integer call() throws Exception {
        Zenoh.initLogFromEnvOr("error");

        Config config = loadConfig(emptyArgs, configFile, connect, listen, noMulticastScouting, mode);
        Selector selector = Selector.tryFrom(this.selectorOpt);

        QueryTarget queryTarget = target != null ? QueryTarget.valueOf(target.toUpperCase()) : QueryTarget.BEST_MATCHING;
        Duration queryTimeout = Duration.ofMillis(timeout);

        Session session = Zenoh.open(config);
        QuerierOptions options = new QuerierOptions();
        options.setTarget(queryTarget);
        options.setTimeout(queryTimeout);
        Querier querier = session.declareQuerier(selector.getKeyExpr(), options);

        performQueries(querier, selector);
        return 0;
    }

    /**
     * Performs queries in an infinite loop, printing responses.
     */
    private void performQueries(Querier querier, Selector selector) throws ZError, InterruptedException {
        for (int idx = 0; idx < Integer.MAX_VALUE; idx++) {
            Thread.sleep(1000);

            String queryPayload = String.format("[%04d] %s", idx, payload != null ? payload : "");
            System.out.println("Querying '" + selector + "' with payload: '" + queryPayload + "'...");

            Querier.GetOptions options = new Querier.GetOptions();
            options.setPayload(queryPayload);
            options.setParameters(selector.getParameters());

            querier.get(this::handleReply, options);
        }
    }

    /**
     * Handles replies received from the query.
     */
    private void handleReply(Reply reply) {
        if (reply instanceof Reply.Success) {
            Reply.Success successReply = (Reply.Success) reply;
            System.out.println(">> Received ('" + successReply.getSample().getKeyExpr() +
                    "': '" + successReply.getSample().getPayload() + "')");
        } else if (reply instanceof Reply.Error) {
            Reply.Error errorReply = (Reply.Error) reply;
            System.out.println(">> Received (ERROR: '" + errorReply.getError() + "')");
        }
    }

    /**
     * ----- Example arguments and private fields -----
     */

    private final Boolean emptyArgs;

    ZQuerier(Boolean emptyArgs) {
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
            names = {"--no-multicast-scouting"},
            description = "Disable the multicast-based scouting mechanism.",
            defaultValue = "false"
    )
    private boolean noMulticastScouting;

    public static void main(String[] args) {
        int exitCode = new CommandLine(new ZQuerier(args.length == 0)).execute(args);
        System.exit(exitCode);
    }
}
