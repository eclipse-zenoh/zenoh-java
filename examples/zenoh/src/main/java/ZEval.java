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

import java.io.InputStreamReader;
import java.util.Collection;
import java.util.Properties;

public class ZEval {

    public static void main(String[] args) {
        String p = "/demo/example/zenoh-java-eval";
        if (args.length > 0) {
            p = args[0];
        }

        String locator = null;
        if (args.length > 1) {
            locator = args[1];
        }

        try {
            Path path = new Path(p);

            System.out.println("Login to Zenoh (locator=" + locator + ")...");
            Zenoh z = Zenoh.login(locator, null);

            System.out.println("Use Workspace on '/'");
            // Note that we use a Workspace with executor here, for our Eval.callback
            // below to be called in a separate thread rather that in Zenoh I/O thread.
            // Thus, the callback can perform some Zenoh operations (e.g.: get)
            Workspace w = z.workspaceWithExecutor(new Path("/"));

            System.out.println("Register eval " + path);
            w.registerEval(path, new Eval() {
                public Value callback(Path p, Properties properties) {
                    // In this Eval function, we choosed to get the name to be returned in the
                    // StringValue in 3 possible ways,
                    // depending the properties specified in the selector. For example, with the
                    // following selectors:
                    // - "/demo/example/zenoh-java-eval" : no properties are set, a default value is
                    // used for the name
                    // - "/demo/example/zenoh-java-eval?(name=Bob)" : "Bob" is used for the name
                    // - "/demo/example/zenoh-java-eval?(name=/demo/example/name)" :
                    // the Eval function does a GET on "/demo/example/name" an uses the 1st result
                    // for the name

                    System.out.printf(">> Processing eval for path %s with properties: %s\n", p, properties);
                    String name = properties.getProperty("name", "Zenoh Java!");

                    if (name.startsWith("/")) {
                        try {
                            System.out.printf("   >> Get name to use from Zenoh at path: %s\n", name);
                            Collection<Data> data = w.get(new Selector(name));
                            if (!data.isEmpty()) {
                                name = data.iterator().next().getValue().toString();
                            }
                        } catch (Throwable e) {
                            System.err.println("Failed to get value from path " + name);
                            e.printStackTrace();
                        }
                    }
                    System.out.printf("   >> Returning string: \"Eval from %s\"\n", name);
                    return new StringValue("Eval from " + name);
                }
            });

            System.out.println("Enter 'q' to quit...\n");
            InputStreamReader stdin = new InputStreamReader(System.in);
            while ((char) stdin.read() != 'q')
                ;

            w.unregisterEval(path);
            z.logout();
        } catch (Throwable e) {
            e.printStackTrace();
        }
    }
}