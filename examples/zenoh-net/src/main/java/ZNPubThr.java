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

class ZNPubThr {

    public static void main(String[] args) {
        String locator = null;
        if (args.length < 1) {
            System.out.println("USAGE:");
            System.out.println("\tZPubThr [I|W]<payload-size> [<zenoh-locator>]");
            System.out.println("\t\tWhere the optional character in front of payload size means:");
            System.out.println("\t\t\tI : use a non-direct ByteBuffer (created via ByteBuffer.allocate())");
            System.out.println("\t\t\tW : use a wrapped ByteBuffer (created via ByteBuffer.wrap())");
            System.out.println("\t\t\tunset : use a direct ByteBuffer");
            System.exit(-1);
        }

        java.nio.ByteBuffer data;
        int len;
        String lenArg = args[0];
        if (lenArg.startsWith("I")) {
            len = Integer.parseInt(lenArg.substring(1));
            data = java.nio.ByteBuffer.allocate(len);
            System.out.println("Running throughput test for payload of " + len + " bytes from a non-direct ByteBuffer");
        } else if (lenArg.startsWith("W")) {
            len = Integer.parseInt(lenArg.substring(1));
            // allocate more than len, to wrap with an offset and test the impact
            byte[] array = new byte[len + 1024];
            data = java.nio.ByteBuffer.wrap(array, 100, len);
            System.out.println("Running throughput test for payload of " + len + " bytes from a wrapped ByteBuffer");
        } else {
            len = Integer.parseInt(lenArg);
            data = java.nio.ByteBuffer.allocateDirect(len);
            System.out.println("Running throughput test for payload of " + len + " bytes from a direct ByteBuffer");
        }

        int posInit = data.position();
        for (int i = 0; i < len; ++i) {
            data.put((byte) (i % 10));
        }
        data.flip();
        data.position(posInit);

        try {
            Session s = Session.open(locator);
            Publisher pub = s.declarePublisher("/test/thr");

            while (true) {
                pub.streamData(data);
            }

        } catch (Throwable e) {
            e.printStackTrace();
        }
    }
}
