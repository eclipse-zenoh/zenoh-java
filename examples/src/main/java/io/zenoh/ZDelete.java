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
import picocli.CommandLine;

import java.util.List;
import java.util.concurrent.Callable;

import static io.zenoh.ConfigKt.loadConfig;

@CommandLine.Command(
        name = "ZDelete",
        mixinStandardHelpOptions = true,
        description = "Zenoh Delete example"
)
public class ZDelete implements Callable<Integer> {

    private final Boolean emptyArgs;

    ZDelete(Boolean emptyArgs) {
        this.emptyArgs = emptyArgs;
    }

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
            names = {"-c", "--config"},
            description = "A configuration file."
    )
    private String configFile;

    @CommandLine.Option(
            names = {"-k", "--key"},
            description = "The key expression to delete [default: demo/example/zenoh-java-delete].",
            defaultValue = "demo/example/zenoh-java-delete"
    )
    private String key;

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

    @Override
    public Integer call() throws ZError {
        Zenoh.initLogFromEnvOr("error");
        System.out.println("Opening session...");
        Config config = loadConfig(emptyArgs, configFile, connect, listen, noMulticastScouting, mode);
        try (Session session = Zenoh.open(config)) {
            KeyExpr keyExpr = KeyExpr.tryFrom(key);
            System.out.println("Deleting resources matching '" + keyExpr + "'...");
            session.delete(keyExpr);
        }
        return 0;
    }

    public static void main(String[] args) {
        int exitCode = new CommandLine(new ZDelete(args.length == 0)).execute(args);
        System.exit(exitCode);
    }
}
