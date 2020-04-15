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
        String uri = "/zenoh/examples/java/stream";
        if (args.length > 0) {
            uri = args[0];
        }

        String value = "Stream from Java!";
        if (args.length > 1) {
            value = args[1];
        }

        String locator = null;
        if (args.length > 2) {
            locator = args[2];
        }

        try {
            System.out.println("Openning session...");
            Session s = Session.open(locator);

            System.out.println("Declaring Publisher on '" + uri + "'...");
            Publisher pub = s.declarePublisher(uri);

            System.out.println("Streaming Data...");
            java.nio.ByteBuffer buf = java.nio.ByteBuffer.allocateDirect(256);
            for (int idx = 0; idx < 100; ++idx) {
                Thread.sleep(1000);
                String str = String.format("[%4d] %s", idx, value);
                buf.put(str.getBytes("UTF-8"));
                System.out.printf("Streaming Data ('%s': '%s')...\n", uri, str);
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
