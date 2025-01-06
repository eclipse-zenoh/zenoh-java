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
import io.zenoh.keyexpr.KeyExpr;
import io.zenoh.pubsub.Publisher;
import io.zenoh.pubsub.PublisherOptions;
import io.zenoh.pubsub.PutOptions;
import io.zenoh.qos.CongestionControl;
import io.zenoh.qos.Reliability;
import picocli.CommandLine;

import java.util.List;
import java.util.concurrent.Callable;

import static io.zenoh.ConfigKt.loadConfig;

@CommandLine.Command(
        name = "ZPub",
        mixinStandardHelpOptions = true,
        description = "Zenoh Pub example"
)
public class ZPub implements Callable<Integer> {

    @Override
    public Integer call() throws ZError, InterruptedException {
        Zenoh.initLogFromEnvOr("error");
        Config config = loadConfig(emptyArgs, configFile, connect, listen, noMulticastScouting, mode);

        System.out.println("Opening session...");
        try (Session session = Zenoh.open(config)) {
            KeyExpr keyExpr = KeyExpr.tryFrom(key);
            System.out.println("Declaring publisher on '" + keyExpr + "'...");

            // A publisher config can optionally be provided.
            PublisherOptions publisherOptions = new PublisherOptions();
            publisherOptions.setEncoding(Encoding.ZENOH_STRING);
            publisherOptions.setCongestionControl(CongestionControl.BLOCK);
            publisherOptions.setReliability(Reliability.RELIABLE);

            // Declare the publisher
            Publisher publisher = session.declarePublisher(keyExpr, publisherOptions);

            System.out.println("Press CTRL-C to quit...");
            ZBytes attachmentBytes = attachment != null ? ZBytes.from(attachment) : null;
            int idx = 0;
            while (true) {
                Thread.sleep(1000);
                String payload = String.format("[%4d] %s", idx, value);
                System.out.println("Putting Data ('" + keyExpr + "': '" + payload + "')...");
                if (attachmentBytes != null) {
                    PutOptions putOptions = new PutOptions();
                    putOptions.setAttachment(attachmentBytes);
                    publisher.put(ZBytes.from(payload), putOptions);
                } else {
                    publisher.put(ZBytes.from(payload));
                }
                idx++;
            }
        }
    }


    /**
     * ----- Example CLI arguments and private fields -----
     */

    private final Boolean emptyArgs;

    ZPub(Boolean emptyArgs) {
        this.emptyArgs = emptyArgs;
    }

    @CommandLine.Option(
            names = {"-k", "--key"},
            description = "The key expression to write to [default: demo/example/zenoh-java-pub].",
            defaultValue = "demo/example/zenoh-java-pub"
    )
    private String key;

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
            names = {"-v", "--value"},
            description = "The value to write. [default: 'Pub from Java!']",
            defaultValue = "Pub from Java!"
    )
    private String value;

    @CommandLine.Option(
            names = {"-a", "--attach"},
            description = "The attachments to add to each put. The key-value pairs are &-separated, and = serves as the separator between key and value."
    )
    private String attachment;

    @CommandLine.Option(
            names = {"--no-multicast-scouting"},
            description = "Disable the multicast-based scouting mechanism.",
            defaultValue = "false"
    )
    private boolean noMulticastScouting;

    public static void main(String[] args) {
        int exitCode = new CommandLine(new ZPub(args.length == 0)).execute(args);
        System.exit(exitCode);
    }
}
