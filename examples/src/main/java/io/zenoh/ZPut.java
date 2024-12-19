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
import io.zenoh.pubsub.PutOptions;
import picocli.CommandLine;

import java.util.List;
import java.util.concurrent.Callable;

import static io.zenoh.ConfigKt.loadConfig;

@CommandLine.Command(
        name = "ZPut",
        mixinStandardHelpOptions = true,
        description = "Zenoh Put example"
)
public class ZPut implements Callable<Integer> {

    @Override
    public Integer call() throws Exception {
        Zenoh.initLogFromEnvOr("error");

        Config config = loadConfig(emptyArgs, configFile, connect, listen, noMulticastScouting, mode);

        System.out.println("Opening session...");
        try (Session session = Zenoh.open(config)) {
            KeyExpr keyExpr = KeyExpr.tryFrom(key);
            System.out.println("Putting Data ('" + keyExpr + "': '" + value + "')...");
            if (attachment != null) {
                var putOptions = new PutOptions();
                putOptions.setAttachment(ZBytes.from(attachment));
                session.put(keyExpr, ZBytes.from(value), putOptions);
            } else {
                session.put(keyExpr, ZBytes.from(value));
            }
        } catch (ZError e) {
            System.err.println("Error during Zenoh operation: " + e.getMessage());
            return 1;
        }

        return 0;
    }


    /**
     * ----- Example CLI arguments and private fields -----
     */

    private final Boolean emptyArgs;

    ZPut(Boolean emptyArgs) {
        this.emptyArgs = emptyArgs;
    }

    @CommandLine.Option(
            names = {"-c", "--config"},
            description = "A configuration file."
    )
    private String configFile;

    @CommandLine.Option(
            names = {"-k", "--key"},
            description = "The key expression to write to [default: demo/example/zenoh-java-put].",
            defaultValue = "demo/example/zenoh-java-put"
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
            names = {"-v", "--value"},
            description = "The value to write. [default: 'Put from Java!'].",
            defaultValue = "Put from Java!"
    )
    private String value;

    @CommandLine.Option(
            names = {"-a", "--attach"},
            description = "The attachment to add to the put. The key-value pairs are &-separated, and = serves as the separator between key and value."
    )
    private String attachment;

    @CommandLine.Option(
            names = {"--no-multicast-scouting"},
            description = "Disable the multicast-based scouting mechanism.",
            defaultValue = "false"
    )
    private boolean noMulticastScouting;

    public static void main(String[] args) {
        int exitCode = new CommandLine(new ZPut(args.length == 0)).execute(args);
        System.exit(exitCode);
    }
}
