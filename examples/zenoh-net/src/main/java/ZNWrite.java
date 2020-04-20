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

import picocli.CommandLine;
import picocli.CommandLine.Option;

class ZNWrite implements Runnable {

    @Option(names = {"-h", "--help"}, usageHelp = true, description = "display this help message")
    private boolean helpRequested = false;

    @Option(names = {"-p", "--path"},
        description = "the path representing the URI.\n  [default: ${DEFAULT-VALUE}]")
    private String path = "/zenoh/examples/java/write/hello";

    @Option(names = {"-l", "--locator"},
        description = "The locator to be used to boostrap the zenoh session. By default dynamic discovery is used")
    private String locator = null;

    @Option(names = {"-m", "--msg"},
        description = "The quote associated with the welcoming resource.\n  [default: ${DEFAULT-VALUE}]")
    private String msg = "Zenitude written from zenoh-net-java!";

    @Override
    public void run() {
        try {
            System.out.println("Openning session...");
            Session s = Session.open(locator);

            System.out.printf("Writing Data ('%s': '%s')...\n", path, msg);
            java.nio.ByteBuffer buf = java.nio.ByteBuffer.wrap(msg.getBytes("UTF-8"));
            s.writeData(path, buf);

            s.close();

        } catch (Throwable e) {
            e.printStackTrace();
        }
    }

    public static void main(String[] args) {
        int exitCode = new CommandLine(new ZNWrite()).execute(args);
        System.exit(exitCode);
    }
}
