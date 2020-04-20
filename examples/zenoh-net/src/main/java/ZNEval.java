/*
 * Copyright (c) 2017, 2020 ADLINK Technology Inc.
 *
 * This program and the accompanying materials are made available under the
 * terms of the Eclipse Public License 2.0 which is available at
 * http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
 * which is available at https://www.apache.org/licenses/LICENSE-2.0.
 *
 * SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
 *
 * Contributors:
 *   ADLINK zenoh team, <zenoh@adlink-labs.tech>
 */
import org.eclipse.zenoh.net.*;

import java.io.InputStreamReader;
import java.nio.ByteBuffer;

import picocli.CommandLine;
import picocli.CommandLine.Option;

class ZNEval implements QueryHandler, Runnable {

    @Option(names = {"-h", "--help"}, usageHelp = true, description = "display this help message")
    private boolean helpRequested = false;

    @Option(names = {"-p", "--path"}, description = "the path representing the URI.\n  [default: ${DEFAULT-VALUE}]")
    private String path = "/zenoh/examples/java/eval";

    @Option(names = {"-l", "--locator"},
        description = "The locator to be used to boostrap the zenoh session. By default dynamic discovery is used")
    private String locator = null;

    // zenoh Eval's callback
    @Override
    public void handleQuery(String rname, String predicate, RepliesSender repliesSender) {
        System.out.printf(">> [Query handler] Handling '%s?%s'\n", rname, predicate);

        ByteBuffer data = ByteBuffer.wrap("Eval from Java!".getBytes());
        Resource[] replies = { new Resource(path, data, 0, 0) };

        repliesSender.sendReplies(replies);
    }

    @Override
    public void run() {
        try {
            System.out.println("Openning session...");
            Session s = Session.open(locator);

            System.out.println("Declaring Eval on '" + path + "'...");
            Eval e = s.declareEval(path, new ZNEval());

            InputStreamReader stdin = new InputStreamReader(System.in);
            while ((char) stdin.read() != 'q')
                ;

            e.undeclare();
            s.close();

        } catch (Throwable e) {
            e.printStackTrace();
        }
    }

    public static void main(String[] args) {
        int exitCode = new CommandLine(new ZNEval()).execute(args);
        System.exit(exitCode);
    }

}
