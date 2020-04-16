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

class ZNStream {

    public static void main(String[] args) {        
        String path = "/zenoh/examples/java/put/hello";
        String value = "Zenitude streamed from zenoh-net-java!";
        String locator = null;

        if (args.length > 0 && (args[0].equals("-h") || args[0].equals("--help"))) {
            System.out.println("USAGE:\n\t ZNStream  [<path>=" + path + "] [<value>] [<locator>=auto]\n\n");
            System.exit(0);
        }
        if (args.length > 0) {
            path = args[0];
        }
        if (args.length > 1) {
            value = args[1];
        }
        if (args.length > 2) {
            locator = args[2];
        }
        
        try {
            System.out.println("Openning session...");
            Session s = Session.open(locator);

            System.out.println("Declaring Publisher on '" + path + "'...");
            Publisher pub = s.declarePublisher(path);

            System.out.println("Streaming Data...");
            java.nio.ByteBuffer buf = java.nio.ByteBuffer.allocateDirect(256);
            for (int idx = 0; idx < 100; ++idx) {
                Thread.sleep(1000);
                String str = String.format("[%4d] %s", idx, value);
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
}
