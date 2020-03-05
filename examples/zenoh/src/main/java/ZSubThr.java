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
import java.util.List;

import org.eclipse.zenoh.Change;
import org.eclipse.zenoh.Listener;
import org.eclipse.zenoh.Path;
import org.eclipse.zenoh.Selector;
import org.eclipse.zenoh.Workspace;
import org.eclipse.zenoh.Zenoh;

class ZSubThr {

    private static final long N = 50000;

    private static long count = 0;
    private static long start;
    private static long stop;

    public static void printStats(long start, long stop) {
        float delta = stop - start;
        float thpt = N / (delta / 1000);
        System.out.format("%f msgs/sec%n", thpt);
    }

    private static class Observer implements Listener {
        public void onChanges(List<Change> changes) {
            if (count == 0) {
                start = System.currentTimeMillis();
                count++;
            } else if (count < N) {
                count++;
            } else {
                stop = System.currentTimeMillis();
                printStats(start, stop);
                System.gc();
                count = 0;
            }
        }
    }

    public static void main(String[] args) {
        String locator = null;
        if (args.length > 0) {
            locator = args[0];
        }

        String s = "/test/thr";
        try {
            Selector selector = new Selector(s);

            System.out.println("Login to Zenoh (locator=" + locator + ")...");
            Zenoh z = Zenoh.login(locator, null);

            System.out.println("Use Workspace on '/'");
            Workspace w = z.workspace(new Path("/"));

            System.out.println("Subscribe on " + selector);
            w.subscribe(selector, new Observer());

            Thread.sleep(60000);

        } catch (Throwable e) {
            e.printStackTrace();
        }
    }
}
