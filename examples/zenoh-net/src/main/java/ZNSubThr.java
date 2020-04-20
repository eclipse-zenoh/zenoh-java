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

import java.nio.ByteBuffer;

import picocli.CommandLine;
import picocli.CommandLine.Option;

class ZNSubThr implements Runnable {

    @Option(names = {"-h", "--help"}, usageHelp = true, description = "display this help message")
    private boolean helpRequested = false;

    @Option(names = {"-p", "--path"},
        description = "The subscriber path.\n  [default: ${DEFAULT-VALUE}]")
    private String path = "/zenoh/examples/throughput/data";

    @Option(names = {"-l", "--locator"},
        description = "The locator to be used to boostrap the zenoh session. By default dynamic discovery is used")
    private String locator = null;

    private static final long N = 100000;

    private static long count = 0;
    private static long start;
    private static long stop;

    public static void printStats(long start, long stop) {
        float delta = stop - start;
        float thpt = N / (delta / 1000);
        System.out.format("%f msgs/sec%n", thpt);
    }

    private static class Listener implements DataHandler {
        public void handleData(String rname, ByteBuffer data, DataInfo info) {
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
            Session s = Session.open(locator);
            Subscriber sub = s.declareSubscriber(path, SubMode.push(), new Listener());

            Thread.sleep(60000);

            sub.undeclare();
            s.close();

        } catch (Throwable e) {
            e.printStackTrace();
        }
    }

    public static void main(String[] args) {
        int exitCode = new CommandLine(new ZNSubThr()).execute(args);
        System.exit(exitCode);
    }
}
