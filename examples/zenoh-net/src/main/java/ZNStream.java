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

class ZNStream implements Runnable {

    @Option(names = {"-h", "--help"}, usageHelp = true, description = "display this help message")
    private boolean helpRequested = false;

    @Option(names = {"-p", "--path"},
        description = "the path representing the URI.\n  [default: ${DEFAULT-VALUE}]")
    private String path = "/zenoh/examples/java/stream/hello";

    @Option(names = {"-l", "--locator"},
        description = "The locator to be used to boostrap the zenoh session. By default dynamic discovery is used")
    private String locator = null;

    @Option(names = {"-m", "--msg"},
        description = "The quote associated with the welcoming resource.\n  [default: ${DEFAULT-VALUE}]")
    private String msg = "Zenitude streamed from zenoh-net-java!";

    @Override
    public void run() {
        try {
            System.out.println("Openning session...");
            Session s = Session.open(locator);

            System.out.println("Declaring Publisher on '" + path + "'...");
            Publisher pub = s.declarePublisher(path);

            System.out.println("Streaming Data...");
            java.nio.ByteBuffer buf = java.nio.ByteBuffer.allocateDirect(256);
            for (int idx = 0; idx < 100; ++idx) {
                Thread.sleep(1000);
                String str = String.format("[%4d] %s", idx, msg);
                buf.put(str.getBytes("UTF-8"));
                System.out.printf("Streaming Data ('%s': '%s')...\n", path, str);
                pub.streamData(buf);
                buf.rewind();
            }

            pub.undeclare();
            s.close();

        } catch (Throwable e) {
            e.printStackTrace();
        }
    }

    public static void main(String[] args) {
        int exitCode = new CommandLine(new ZNStream()).execute(args);
        System.exit(exitCode);
    }
}
