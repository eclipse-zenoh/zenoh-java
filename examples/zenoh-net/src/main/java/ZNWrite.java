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
        String uri = "/zenoh/examples/java/write";
        if (args.length > 0) {
            uri = args[0];
        }

        String value = "Write from Java!";
        if (args.length > 1) {
            value = args[1];
        }

        String locator = null;
        if (args.length > 3) {
            locator = args[3];
        }

        try {
            System.out.println("Openning session...");
            Session s = Session.open(locator);

            System.out.printf("Writing Data ('%s': '%s')...\n", uri, value);
            java.nio.ByteBuffer buf = java.nio.ByteBuffer.wrap(value.getBytes("UTF-8"));
            s.writeData(uri, buf);

            s.close();

        } catch (Throwable e) {
            e.printStackTrace();
        }
    }
}
