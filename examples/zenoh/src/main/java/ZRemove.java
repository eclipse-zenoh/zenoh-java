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

public class ZRemove {

    public static void main(String[] args) {
        // If not specified as 1st argument, use a relative path (to the workspace
        // below): "zenoh-java-put"
        String path = "zenoh-java-put";
        if (args.length > 0) {
            path = args[0];
        }

        String locator = null;
        if (args.length > 1) {
            locator = args[1];
        }

        try {
            Path p = new Path(path);

            System.out.println("Login to Zenoh (locator=" + locator + ")...");
            Zenoh z = Zenoh.login(locator, null);

            System.out.println("Use Workspace on '/demo/example'");
            Workspace w = z.workspace(new Path("/demo/example"));

            System.out.println("Remove " + p);
            w.remove(p);

            z.logout();
        } catch (Throwable e) {
            e.printStackTrace();
        }
    }
}