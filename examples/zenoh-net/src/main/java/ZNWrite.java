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

class ZNWrite {

    public static void main(String[] args) {
        String path = "/zenoh/examples/java/put/hello";
        String value = "Zenitude wrote from zenoh-net-java!";
        String locator = null;

        if (args.length > 0 && (args[0].equals("-h") || args[0].equals("--help"))) {
            System.out.println("USAGE:\n\t ZNWrite  [<path>=" + path + "] [<value>] [<locator>=auto]\n\n");
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

            System.out.printf("Writing Data ('%s': '%s')...\n", path, value);
            java.nio.ByteBuffer buf = java.nio.ByteBuffer.wrap(value.getBytes("UTF-8"));
            s.writeData(path, buf);

            s.close();

        } catch (Throwable e) {
            e.printStackTrace();
        }
    }
}
