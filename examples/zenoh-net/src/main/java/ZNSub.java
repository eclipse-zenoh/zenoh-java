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
import java.io.UnsupportedEncodingException;
import java.nio.ByteBuffer;

class ZNSub {

    private static class Listener implements DataHandler {
        public void handleData(String rname, ByteBuffer data, DataInfo info) {
            try {
                byte[] buf = new byte[data.remaining()];
                data.get(buf);
                String str = new String(buf, "UTF-8");
                System.out.printf(">> [Subscription listener] Received ('%s': '%s')\n", rname, str);
            } catch (UnsupportedEncodingException e) {
                System.out.printf(">> [Subscription listener] Received ('%s': '%s')\n", rname, data.toString());
            }
        }
    }

    public static void main(String[] args) {
        String uri = "/zenoh/examples/**";
        if (args.length > 0) {
            uri = args[0];
        }

        String locator = null;
        if (args.length > 1) {
            locator = args[1];
        }

        try {
            System.out.println("Openning session...");
            Session s = Session.open(locator);

            System.out.println("Declaring Subscriber on '" + uri + "'...");
            Subscriber sub = s.declareSubscriber(uri, SubMode.push(), new Listener());

            InputStreamReader stdin = new InputStreamReader(System.in);
            while ((char) stdin.read() != 'q')
                ;

            sub.undeclare();
            s.close();

        } catch (Throwable e) {
            e.printStackTrace();
        }
    }
}
