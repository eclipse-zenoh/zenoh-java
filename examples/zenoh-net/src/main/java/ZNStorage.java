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
import java.util.Map;
import java.util.Vector;
import java.util.HashMap;
import java.util.List;

class ZNStorage implements StorageHandler {

    private Map<String, ByteBuffer> stored = new HashMap<String, ByteBuffer>();

    public void handleData(String rname, ByteBuffer data, DataInfo info) {
        try {
            byte[] buf = new byte[data.remaining()];
            data.get(buf);
            String str = new String(buf, "UTF-8");
            System.out.printf(">> [Subscription listener] Received ('%s': '%s')\n", rname, str);
        } catch (UnsupportedEncodingException e) {
            System.out.printf(">> [Subscription listener] Received ('%s': '%s')\n", rname, data.toString());
        }
        data.rewind();
        this.stored.put(rname, data);
    }

    public void handleQuery(String rname, String predicate, RepliesSender repliesSender) {
        System.out.printf(">> [Query handler   ] Handling '%s?%s'\n", rname, predicate);

        List<Resource> replies = new Vector<Resource>();
        for (Map.Entry<String, ByteBuffer> entry : stored.entrySet()) {
            if (Rname.intersect(rname, entry.getKey())) {
                replies.add(new Resource(entry.getKey(), entry.getValue(), 0, 0));
            }
        }
        repliesSender.sendReplies(replies.toArray(new Resource[replies.size()]));
    }

    public static void main(String[] args) {
        String uri = "/demo/example/**";
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

            System.out.println("Declaring Storage on '" + uri + "'...");
            Storage sto = s.declareStorage(uri, new ZNStorage());

            InputStreamReader stdin = new InputStreamReader(System.in);
            while ((char) stdin.read() != 'q')
                ;

            sto.undeclare();
            s.close();
        } catch (Throwable e) {
            e.printStackTrace();
        }
    }
}
