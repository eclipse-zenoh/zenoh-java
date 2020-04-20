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
import org.eclipse.zenoh.Selector;
import org.eclipse.zenoh.Workspace;
import org.eclipse.zenoh.Zenoh;

import picocli.CommandLine;
import picocli.CommandLine.Option;

class ZSubThr implements Runnable {

    @Option(names = {"-h", "--help"}, usageHelp = true, description = "display this help message")
    private boolean helpRequested = false;

    @Option(names = {"-p", "--path"},
        description = "The subscriber path.\n  [default: ${DEFAULT-VALUE}]")
    private String path = "/zenoh/examples/throughput/data";

    @Option(names = {"-l", "--locator"},
        description = "The locator to be used to boostrap the zenoh session. By default dynamic discovery is used")
    private String locator = null;

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

    @Override
    public void run() {
        try {
            Selector selector = new Selector(path);

            System.out.println("Login to Zenoh (locator=" + locator + ")...");
            Zenoh z = Zenoh.login(locator);

            Workspace w = z.workspace();

            System.out.println("Subscribe on " + selector);
            w.subscribe(selector, new Observer());

            Thread.sleep(60000);

        } catch (Throwable e) {
            e.printStackTrace();
        }
    }

    public static void main(String[] args) {
        int exitCode = new CommandLine(new ZSubThr()).execute(args);
        System.exit(exitCode);
    }
}
