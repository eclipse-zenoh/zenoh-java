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

import java.io.BufferedReader;
import java.io.InputStreamReader;

import org.eclipse.zenoh.*;

public class ZPutFloat {

    public static void main(String[] args) {
        String path = "/zenoh/examples/native/float";
        String locator = null;

        if (args.length > 0 && (args[0].equals("-h") || args[0].equals("--help"))) {
            System.out.println("USAGE:\n\t ZPut  [<path>=" + path + "] [<value>] [<locator>=auto]\n\n");
            System.exit(0);
        }
        if (args.length > 0) {
            path = args[0];
        }
        if (args.length > 1) {
            locator = args[1];
        }

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
}