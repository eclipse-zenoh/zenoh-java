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
import java.util.HashMap;
import java.util.Map;

import org.eclipse.zenoh.net.*;

import picocli.CommandLine;
import picocli.CommandLine.Option;

class ZNInfo implements Runnable {

    @Option(names = {"-h", "--help"}, usageHelp = true, description = "display this help message")
    private boolean helpRequested = false;

    @Option(names = {"-l", "--locator"},
        description = "The locator to be used to boostrap the zenoh session. By default dynamic discovery is used")
    private String locator = null;

    private static final char[] HEX_DIGITS = "0123456789abcdef".toCharArray();

    public static String hexdump(byte[] bytes) {
        StringBuffer sb = new StringBuffer();
        for (byte b : bytes) {
            sb.append(HEX_DIGITS[(b & 0xF0) >> 4]).append(HEX_DIGITS[b & 0x0F]);
        }
        return sb.toString();
    }

    @Override
    public void run() {
        try {
            System.out.println("Openning session...");
            Map<Integer, byte[]> properties = new HashMap<Integer, byte[]>(2);
            properties.put(ZNet.USER_KEY, "user".getBytes());
            properties.put(ZNet.PASSWD_KEY, "password".getBytes());
            Session s = Session.open(locator, properties);

            Map<Integer, byte[]> info = s.info();
            System.out.println("LOCATOR  : " + new String(info.get(ZNet.INFO_PEER_KEY)));
            System.out.println("PID      : " + hexdump(info.get(ZNet.INFO_PID_KEY)));
            System.out.println("PEER PID : " + hexdump(info.get(ZNet.INFO_PEER_PID_KEY)));

            s.close();
        } catch (Throwable e) {
            e.printStackTrace();
        }
    }

    public static void main(String[] args) {
        int exitCode = new CommandLine(new ZNInfo()).execute(args);
        System.exit(exitCode);
    }
}
