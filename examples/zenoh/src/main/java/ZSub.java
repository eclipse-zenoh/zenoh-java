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
import java.util.List;

import picocli.CommandLine;
import picocli.CommandLine.Option;

public class ZSub implements Runnable {

    @Option(names = {"-h", "--help"}, usageHelp = true, description = "display this help message")
    private boolean helpRequested = false;

    @Option(names = {"-s", "--selector"},
        description = "The selector specifying the subscription.\n  [default: ${DEFAULT-VALUE}]")
    private String selector = "/zenoh/examples/**";

    @Option(names = {"-l", "--locator"},
        description = "The locator to be used to boostrap the zenoh session. By default dynamic discovery is used")
    private String locator = null;

    @Override
    public void run() {
        try {
            Selector s = new Selector(selector);

            System.out.println("Login to Zenoh (locator=" + locator + ")...");
            Zenoh z = Zenoh.login(locator);
        
            Workspace w = z.workspace();

            System.out.println("Subscribe on " + selector);
            SubscriptionId subid = w.subscribe(s, new Listener() {
                public void onChanges(List<Change> changes) {
                    for (Change c : changes) {
                        switch (c.getKind()) {
                        case PUT:
                            System.out.printf(">> [Subscription listener] Received PUT on '%s': '%s')\n", c.getPath(),
                                    c.getValue());
                            break;
                        case UPDATE:
                            System.out.printf(">> [Subscription listener] Received UPDATE on '%s': '%s')\n",
                                    c.getPath(), c.getValue());
                            break;
                        case REMOVE:
                            System.out.printf(">> [Subscription listener] Received REMOVE on '%s')\n", c.getPath());
                            break;
                        default:
                            System.err.printf(
                                    ">> [Subscription listener] Received unkown operation with kind '%s' on '%s')\n",
                                    c.getKind(), c.getPath());
                            break;
                        }
                    }
                }
            });

            System.out.println("Enter 'q' to quit...\n");
            InputStreamReader stdin = new InputStreamReader(System.in);
            while ((char) stdin.read() != 'q')
                ;

            w.unsubscribe(subid);
            z.logout();
        } catch (Throwable e) {
            e.printStackTrace();
        }
    }

    public static void main(String[] args) {
        int exitCode = new CommandLine(new ZSub()).execute(args);
        System.exit(exitCode);
    }
}