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

class ZNEval implements QueryHandler {

    private static String uri = "/zenoh/examples/java/eval";

    public void handleQuery(String rname, String predicate, RepliesSender repliesSender) {
        System.out.printf(">> [Query handler] Handling '%s?%s'\n", rname, predicate);

        ByteBuffer data = ByteBuffer.wrap("Eval from Java!".getBytes());
        Resource[] replies = { new Resource(uri, data, 0, 0) };

        repliesSender.sendReplies(replies);
    }

    public static void main(String[] args) {
        String path = "/zenoh/examples/java/eval";
        String locator = null;
        if (args.length > 0 && (args[0].equals("-h") || args[0].equals("--help"))) {
            System.out.println("USAGE:\n\t ZNEval  [<path>=" + path + "] [<locator>=auto]\n\n");
            System.exit(0);
        }
        if (args.length > 0) {
            path = args[0];
        }
        if (args.length > 1) {
            locator = args[1];
        }

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
}
