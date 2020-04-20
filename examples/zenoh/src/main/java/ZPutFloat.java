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


import org.eclipse.zenoh.*;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import picocli.CommandLine;
import picocli.CommandLine.Option;

public class ZPutFloat implements Runnable {

    @Option(names = {"-h", "--help"}, usageHelp = true, description = "display this help message")
    private boolean helpRequested = false;

    @Option(names = {"-p", "--path"},
        description = "the path representing the float resource.\n  [default: ${DEFAULT-VALUE}]")
    private String path = "/zenoh/examples/native/float";

    @Option(names = {"-l", "--locator"},
        description = "The locator to be used to boostrap the zenoh session. By default dynamic discovery is used")
    private String locator = null;

    @Override
    public void run() {
        try {
            Path p = new Path(path);

            Zenoh z = Zenoh.login(locator);
            Workspace w = z.workspace();

            BufferedReader reader = new BufferedReader(new InputStreamReader(System.in));
            String v = null;
            while (! ".".equals(v)) {
                System.out.print("Insert value ('.' to exit): ");
                v = reader.readLine();
                try {
                    w.put(p, Float.parseFloat(v));
                } catch (NumberFormatException e) {
                    System.err.println("Invalid float!");
                }
            }

            z.logout();
        } catch (Throwable e) {
            e.printStackTrace();
        }
    }

    public static void main(String[] args) {
        int exitCode = new CommandLine(new ZPutFloat()).execute(args);
        System.exit(exitCode);
    }
}