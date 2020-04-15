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

class ZNQuery {

    private static class Handler implements ReplyHandler {

        public void handleReply(ReplyValue reply) {
            switch (reply.getKind()) {
            case ZN_STORAGE_DATA:
            case ZN_EVAL_DATA:
                java.nio.ByteBuffer data = reply.getData();
                try {
                    byte[] buf = new byte[data.remaining()];
                    data.get(buf);
                    String s = new String(buf, "UTF-8");
                    if (reply.getKind() == ReplyValue.Kind.ZN_STORAGE_DATA) {
                        System.out.printf(">> [Reply handler] received -Storage Data- ('%s': '%s')\n", reply.getRname(),
                                s);
                    } else {
                        System.out.printf(">> [Reply handler] received -Eval Data-    ('%s': '%s')\n", reply.getRname(),
                                s);
                    }
                } catch (java.io.UnsupportedEncodingException e) {
                    System.out.println(">> [Reply handler] error decoding data: " + e);
                }
                break;
            case ZN_STORAGE_FINAL:
                System.out.println(">> [Reply handler] received -Storage Final-");
                break;
            case ZN_EVAL_FINAL:
                System.out.println(">> [Reply handler] received -Eval Final-");
                break;
            case ZN_REPLY_FINAL:
                System.out.println(">> [Reply handler] received -Reply Final-");
                break;
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

            System.out.println("Send query '" + uri + "'...");
            s.query(uri, "", new Handler(), QueryDest.all(), QueryDest.all());

            Thread.sleep(1000);

            s.close();
        } catch (Throwable e) {
            e.printStackTrace();
        }
    }
}
