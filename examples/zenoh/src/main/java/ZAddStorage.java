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

import java.util.Properties;

public class ZAddStorage {

    public static void main(String[] args) {
        String selector = "/zenoh/examples/**";
        String storageId = "zenoh-examples-storage";
        String locator = null;

        if (args.length > 0 && (args[0].equals("-h") || args[0].equals("--help"))) {
            System.out.println("USAGE:\n\t ZAddStorage  [<selector>=" + selector + "] [<storage-id>=" + storageId + "] [<locator>=auto]\n\n");
            System.exit(0);
        }
        if (args.length > 0) {
            selector = args[0];
        }
        if (args.length > 1) {
            storageId = args[1];
        }
        if (args.length > 2) {
            locator = args[2];
        }

        try {
            System.out.println("Login to Zenoh (locator=" + locator + ")...");
            Zenoh z = Zenoh.login(locator);

            Admin admin = z.admin();

            System.out.println("Add storage " + storageId + " with selector " + selector);
            Properties p = new Properties();
            p.setProperty("selector", selector);
            admin.addStorage(storageId, p);

            z.logout();
        } catch (Throwable e) {
            e.printStackTrace();
        }
    }

}
