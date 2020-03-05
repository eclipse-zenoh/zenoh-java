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
package org.eclipse.zenoh;

import java.nio.charset.Charset;
import java.util.HashMap;
import java.util.Map;
import java.util.Properties;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import org.eclipse.zenoh.core.ZException;
import org.eclipse.zenoh.net.Session;
import org.eclipse.zenoh.net.ZNet;

/**
 * The Zenoh client API.
 */
public class Zenoh {

    private static final String PROP_USER = "user";
    private static final String PROP_PASSWORD = "password";

    private static final Logger LOG = LoggerFactory.getLogger("org.eclipse.zenoh");
    private static final char[] HEX_DIGITS = "0123456789abcdef".toCharArray();
    private static final Charset UTF8 = Charset.forName("UTF-8");

    private static String hexdump(byte[] bytes) {
        StringBuffer sb = new StringBuffer();
        for (int i = 0; i < bytes.length; ++i) {
            sb.append(HEX_DIGITS[(bytes[i] & 0xF0) >>> 4]).append(HEX_DIGITS[bytes[i] & 0x0F]);
        }
        return sb.toString();
    }

    private Session session;
    private ExecutorService threadPool;
    private Admin admin;

    private Zenoh(Session session, String zid) {
        this.session = session;
        this.threadPool = Executors.newCachedThreadPool();
        Workspace adminWs = new Workspace(new Path("/@"), session, threadPool);
        this.admin = new Admin(adminWs, zid);
    }

    /**
     * Establish a zenoh session via a provided locator. Locator is a string
     * representing the network endpoint to which establish the session. If the
     * provided locator is <tt>null</tt>, login will perform some dynamic discovery
     * and try to establish the session automatically. When not <tt>null</tt>, the
     * locator must have the format: {@code tcp/<ip>:<port>} (for instance
     * {@code tcp/127.0.0.1:7447}).
     *
     * @param locator    the locator or <tt>null</tt>.
     * @param properties the Properties to be used for this session (e.g. "user",
     *                   "password"...). Can be <tt>null</tt>.
     * @return a {@link Zenoh} object.
     * @throws ZException if login fails.
     */
    public static Zenoh login(String locator, Properties properties) throws ZException {
        try {
            LOG.debug("Establishing session to Zenoh router {}", locator);
            Session s;
            if (properties == null) {
                s = Session.open(locator);
            } else {
                s = Session.open(locator, getSessionProperties(properties));
            }

            Map<Integer, byte[]> props = s.info();
            byte[] buf = props.get(ZNet.INFO_PEER_PID_KEY);
            if (buf == null) {
                throw new ZException("Failed to retrieve Zenoh id from Session info");
            }
            String zid = hexdump(buf);
            LOG.info("Session established with Zenoh router {}", zid);
            return new Zenoh(s, zid);

        } catch (ZException e) {
            LOG.warn("Failed to establish session to {}", locator, e);
            throw new ZException("Login failed to " + locator, e);
        }
    }

    private static Map<Integer, byte[]> getSessionProperties(Properties properties) {
        Map<Integer, byte[]> zprops = new HashMap<Integer, byte[]>();

        if (properties.containsKey(PROP_USER))
            zprops.put(ZNet.USER_KEY, properties.getProperty(PROP_USER).getBytes(UTF8));
        if (properties.containsKey(PROP_PASSWORD))
            zprops.put(ZNet.PASSWD_KEY, properties.getProperty(PROP_PASSWORD).getBytes(UTF8));

        return zprops;
    }

    /**
     * Terminates the Zenoh session.
     */
    public void logout() throws ZException {
        threadPool.shutdown();
        try {
            session.close();
            this.session = null;
        } catch (ZException e) {
            throw new ZException("Error during logout", e);
        }
    }

    /**
     * Creates a Workspace using the provided path. All relative {@link Selector} or
     * {@link Path} used with this Workspace will be relative to this path.
     * <p>
     * Notice that all subscription listeners and eval callbacks declared in this
     * workspace will be executed by the I/O thread. This implies that no long
     * operations or other call to Zenoh shall be performed in those callbacks.
     *
     * @param path the Workspace's path.
     * @return a {@link Workspace}.
     */
    public Workspace workspace(Path path) {
        return new Workspace(path, session, null);
    }

    /**
     * Creates a Workspace using the provided path. All relative {@link Selector} or
     * {@link Path} used with this Workspace will be relative to this path.
     * <p>
     * Notice that all subscription listeners and eval callbacks declared in this
     * workspace will be executed by a CachedThreadPool. This is useful when
     * listeners and/or callbacks need to perform long operations or need to call
     * other Zenoh operations.
     *
     * @param path the Workspace's path.
     * @return a {@link Workspace}.
     */
    public Workspace workspaceWithExecutor(Path path) {
        return new Workspace(path, session, threadPool);
    }

    /**
     * Returns the {@link Admin} object that provides helper operations to
     * administer Zenoh.
     */
    public Admin admin() {
        return admin;
    }

}
