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

import org.eclipse.zenoh.core.Timestamp;
import org.eclipse.zenoh.core.ZException;
import org.eclipse.zenoh.net.*;

import java.util.Collection;
import java.util.List;
import java.util.ArrayList;
import java.util.LinkedList;
import java.util.Map;
import java.util.Hashtable;
import java.util.Properties;
import java.util.SortedSet;
import java.util.TreeSet;
import java.util.concurrent.ExecutorService;
import java.nio.ByteBuffer;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * A Workspace to operate on Zenoh.
 */
public class Workspace {

    private static final Logger LOG = LoggerFactory.getLogger("org.eclipse.zenoh");

    private static final short KIND_PUT = (short) 0x0;
    private static final short KIND_UPDATE = (short) 0x1;
    private static final short KIND_REMOVE = (short) 0x2;
    private static final ByteBuffer EMPTY_BUF = ByteBuffer.allocateDirect(0);

    private Path path;
    private Session session;
    private ExecutorService threadPool;
    private Map<Path, org.eclipse.zenoh.net.Eval> evals;


    protected Workspace(Path path, Session session, ExecutorService threadPool) {
        this.path = path;
        this.session = session;
        this.threadPool = threadPool;
        this.evals = new Hashtable<Path, org.eclipse.zenoh.net.Eval>();
    }

    private Path toAsbsolutePath(Path p) {
        if (p.isRelative()) {
            return p.addPrefix(this.path);
        } else {
            return p;
        }
    }

    private Selector toAsbsoluteSelector(Selector s) {
        if (s.isRelative()) {
            return s.addPrefix(this.path);
        } else {
            return s;
        }
    }

    /**
     * Put a path/value into Zenoh.
     *
     * @param path  the string representing the path. Can be absolute or relative to the workspace.
     * @param value the value {@link java.lang.String}.
     * @throws ZException if put failed.
     */
    public void put(String path, String value) throws ZException {
        this.put(new Path(path), new StringValue(value));
    }

    /**
     * Put a path/value into Zenoh.
     *
     * @param path  the string representing the path. Can be absolute or relative to the workspace.
     * @param value the int value
     * @throws ZException if put failed.
     */
    public void put(String path, int value) throws ZException {
        this.put(new Path(path), new IntValue(value));
    }

    /**
     * Put a path/value into Zenoh.
     *
     * @param path  the string representing the path. Can be absolute or relative to the workspace.
     * @param value the float value.
     * @throws ZException if put failed.
     */

    public void put(String path, float value) throws ZException {
        this.put(new Path(path), new FloatValue(value));
    }

    /**
     * Put a path/value into Zenoh.
     *
     * @param path  the {@link Path}. Can be absolute or relative to the workspace.
     * @param value the {@link Value}.
     * @throws ZException if put failed.
     */
    public void put(Path path, Value value) throws ZException {
        path = toAsbsolutePath(path);
        LOG.debug("Put on {} of {}", path, value);
        try {
            ByteBuffer data = value.encode();
            session.writeData(path.toString(), data, value.getEncoding().getFlag(), KIND_PUT);
        } catch (ZException e) {
            throw new ZException("Put on " + path + " failed", e);
        }
    }

    /**
     * Update a path/value into Zenoh.
     *
     * @param path  the {@link Path}. Can be absolute or relative to the workspace.
     * @param value a delta to be applied on the existing value.
     * @throws ZException if update failed.
     */
    public void update(Path path, Value value) throws ZException {
        path = toAsbsolutePath(path);
        LOG.debug("Update on {} of {}", path, value);
        try {
            ByteBuffer data = value.encode();
            session.writeData(path.toString(), data, value.getEncoding().getFlag(), KIND_UPDATE);
        } catch (ZException e) {
            throw new ZException("Update on " + path + " failed", e);
        }
    }

    /**
     * Remove a path/value from Zenoh.
     *
     * @param path the {@link Path} to be removed. Can be absolute or relative to
     *             the workspace.
     * @throws ZException if remove failed.
     */
    public void remove(Path path) throws ZException {
        path = toAsbsolutePath(path);
        LOG.debug("Remove on {}", path);
        try {
            session.writeData(path.toString(), EMPTY_BUF, (short) 0, KIND_REMOVE);
        } catch (ZException e) {
            throw new ZException("Remove on " + path + " failed", e);
        }
    }

    // Timestamp generated locally when not available in received Data
    // @TODO: remove this when we're sure Data always come with a Timestamp.
    private static class LocalTimestamp extends Timestamp {
        private static final byte[] zeroId = { 0x00 };

        LocalTimestamp() {
            super(currentTime(), zeroId);
        }

        private static long currentTime() {
            long now = System.currentTimeMillis();
            long sec = (now / 1000) << 32;
            float frac = (((float) (now % 1000)) / 1000) * 0x100000000L;
            return sec + (long) frac;
        }
    }

    /**
     * Get a selection of path/value from Zenoh.
     *
     * @param selector the {@link Selector} expressing the selection.
     * @return a collection of path/value.
     * @throws ZException if get failed.
     */
    public Collection<Data> get(Selector selector) throws ZException {
        final Selector s = toAsbsoluteSelector(selector);
        LOG.debug("Get on {}", s);
        try {
            final Map<Path, SortedSet<Data>> map = new Hashtable<Path, SortedSet<Data>>();
            final java.util.concurrent.atomic.AtomicBoolean queryFinished = new java.util.concurrent.atomic.AtomicBoolean(
                    false);

            session.query(s.getPath(), s.getOptionalPart(), new ReplyHandler() {
                public void handleReply(ReplyValue reply) {
                    switch (reply.getKind()) {
                    case ZN_STORAGE_DATA:
                    case ZN_EVAL_DATA:
                        Path path = new Path(reply.getRname());
                        ByteBuffer data = reply.getData();
                        short encodingFlag = (short) reply.getInfo().getEncoding();
                        if (reply.getKind() == ReplyValue.Kind.ZN_STORAGE_DATA) {
                            LOG.debug("Get on {} => ZN_STORAGE_DATA {} : {} ({} bytes)", s, path,
                                    reply.getInfo().getTimestamp(), data.remaining());
                        } else {
                            LOG.debug("Get on {} => ZN_EVAL_DATA {} : {} ({} bytes)", s, path,
                                    reply.getInfo().getTimestamp(), data.remaining());
                        }
                        try {
                            Value value = Encoding.fromFlag(encodingFlag).getDecoder().decode(data);
                            Timestamp ts = reply.getInfo().getTimestamp();
                            // @TODO: remove this when we're sure Data always come with a Timestamp.
                            if (ts == null) {
                                LOG.warn(
                                        "Get on {}: received data for {} without timestamp. Adding it with current date.",
                                        s, path);
                                ts = new LocalTimestamp();
                            }
                            Data d = new Data(path, value, ts);
                            if (!map.containsKey(path)) {
                                map.put(path, new TreeSet<Data>());
                            }
                            map.get(path).add(d);
                        } catch (ZException e) {
                            LOG.warn("Get on {}: error decoding reply {} : {}", s, reply.getRname(), e);
                        }
                        break;
                    case ZN_STORAGE_FINAL:
                        LOG.trace("Get on {} => ZN_STORAGE_FINAL", s);
                        break;
                    case ZN_EVAL_FINAL:
                        LOG.trace("Get on {} => ZN_EVAL_FINAL", s);
                        break;
                    case ZN_REPLY_FINAL:
                        LOG.trace("Get on {} => ZN_REPLY_FINAL", s);
                        synchronized (map) {
                            queryFinished.set(true);
                            map.notify();
                        }
                        break;
                    }
                }
            });

            synchronized (map) {
                while (!queryFinished.get()) {
                    try {
                        map.wait();
                    } catch (InterruptedException e) {
                        // ignore
                    }
                }
            }

            Collection<Data> results = new LinkedList<Data>();

            if (isSelectorForSeries(selector)) {
                // return all entries
                for (SortedSet<Data> l : map.values()) {
                    for (Data e : l) {
                        results.add(e);
                    }
                }
            } else {
                // return only the latest data for each path
                for (SortedSet<Data> l : map.values()) {
                    results.add(l.last());
                }
            }

            return results;

        } catch (ZException e) {
            throw new ZException("Get on " + selector + " failed", e);
        }
    }

    private boolean isSelectorForSeries(Selector sel) {
        // search for starttime or stoptime property in selector
        String[] props = sel.getProperties().split(";");
        for (String p : props) {
            if (p.startsWith("starttime") || p.startsWith("stoptime")) {
                return true;
            }
        }
        return false;
    }

    /**
     * Subscribe to a selection of path/value from Zenoh.
     *
     * @param selector the {@link Selector} expressing the selection.
     * @param listener the {@link Listener} that will be called for each change of a
     *                 path/value matching the selection.
     * @return a {@link SubscriptionId}.
     * @throws ZException if subscribe failed.
     */
    public SubscriptionId subscribe(Selector selector, Listener listener) throws ZException {
        final Selector s = toAsbsoluteSelector(selector);
        LOG.debug("subscribe on {}", selector);
        try {
            Subscriber sub = session.declareSubscriber(s.getPath(), SubMode.push(), new DataHandler() {
                public void handleData(String rname, ByteBuffer data, DataInfo info) {
                    LOG.debug("subscribe on {} : received notif for {} (kind:{})", s, rname, info.getKind());
                    // TODO: list of more than 1 change when available in zenoh-c
                    List<Change> changes = new ArrayList<Change>(1);

                    try {
                        Path path = new Path(rname);
                        Change.Kind kind = Change.Kind.fromInt(info.getKind());
                        short encodingFlag = (short) info.getEncoding();
                        Timestamp timestamp = info.getTimestamp();
                        Value value = null;
                        if (kind != Change.Kind.REMOVE) {
                            value = Encoding.fromFlag(encodingFlag).getDecoder().decode(data);
                        }
                        Change change = new Change(path, kind, timestamp, value);
                        changes.add(change);
                    } catch (ZException e) {
                        LOG.warn("subscribe on {}: error decoding change for {} : {}", s, rname, e);
                    }

                    if (threadPool != null) {
                        threadPool.execute(new Runnable() {
                            @Override
                            public void run() {
                                try {
                                    listener.onChanges(changes);
                                } catch (Throwable e) {
                                    LOG.warn("subscribe on {} : error receiving notification for {} : {}", s, rname, e);
                                    LOG.debug("Stack trace: ", e);
                                }
                            }
                        });
                    } else {
                        try {
                            listener.onChanges(changes);
                        } catch (Throwable e) {
                            LOG.warn("subscribe on {} : error receiving notification for {} : {}", s, rname, e);
                            LOG.debug("Stack trace: ", e);
                        }
                    }
                }
            });
            return new SubscriptionId(sub);

        } catch (ZException e) {
            throw new ZException("Subscribe on " + selector + " failed", e);
        }
    }

    /**
     * Unregisters a previous subscription.
     *
     * @param subid the {@link SubscriptionId} to unregister
     * @throws ZException if unsusbscribe failed.
     */
    public void unsubscribe(SubscriptionId subid) throws ZException {
        try {
            subid.getZSubscriber().undeclare();
        } catch (ZException e) {
            throw new ZException("Unsubscribe failed", e);
        }
    }

    private static final Resource[] EMPTY_EVAL_REPLY = new Resource[0];

    /**
     * Registers an evaluation function under the provided {@link Path}. The
     * function will be evaluated in a dedicated thread, and thus may call any other
     * Workspace operation.
     *
     * @param path the {@link Path} where the function can be triggered using
     *             {@link #get(Selector)}
     * @param eval the evaluation function
     * @throws ZException if registration failed.
     */
    public void registerEval(Path path, Eval eval) throws ZException {
        final Path p = toAsbsolutePath(path);
        LOG.debug("registerEval on {}", p);
        try {
            QueryHandler qh = new QueryHandler() {
                public void handleQuery(String rname, String predicate, RepliesSender repliesSender) {
                    LOG.debug("Registered eval on {} handling query {}?{}", p, rname, predicate);
                    Selector s = new Selector(rname + "?" + predicate);
                    if (threadPool != null) {
                        threadPool.execute(new Runnable() {
                            @Override
                            public void run() {
                                try {
                                    Value v = eval.callback(p, predicateToProperties(s.getProperties()));
                                    LOG.debug("Registered eval on {} handling query {}?{} returns: {}", p, rname,
                                            predicate, v);
                                    repliesSender.sendReplies(new Resource[] { new Resource(p.toString(), v.encode(),
                                            v.getEncoding().getFlag(), Change.Kind.PUT.value()) });
                                } catch (Throwable e) {
                                    LOG.warn(
                                            "Registered eval on {} caught an exception while handling query {} {} : {}",
                                            p, rname, predicate, e);
                                    LOG.debug("Stack trace: ", e);
                                    repliesSender.sendReplies(EMPTY_EVAL_REPLY);
                                }
                            }
                        });
                    } else {
                        try {
                            Value v = eval.callback(p, predicateToProperties(s.getProperties()));
                            LOG.debug("Registered eval on {} handling query {}?{} returns: {}", p, rname, predicate, v);
                            repliesSender.sendReplies(new Resource[] { new Resource(p.toString(), v.encode(),
                                    v.getEncoding().getFlag(), Change.Kind.PUT.value()) });
                        } catch (Throwable e) {
                            LOG.warn("Registered eval on {} caught an exception while handling query {} {} : {}", p,
                                    rname, predicate, e);
                            LOG.debug("Stack trace: ", e);
                            repliesSender.sendReplies(EMPTY_EVAL_REPLY);
                        }
                    }
                }
            };

            org.eclipse.zenoh.net.Eval e = session.declareEval(p.toString(), qh);
            evals.put(p, e);

        } catch (ZException e) {
            throw new ZException("registerEval on " + p + " failed", e);
        }

    }

    /**
     * Unregister a previously registered evaluation function.
     *
     * @param path the {@link Path} where the function has been registered
     * @throws ZException if unregistration failed.
     */
    public void unregisterEval(Path path) throws ZException {
        org.eclipse.zenoh.net.Eval e = evals.remove(path);
        if (e != null) {
            try {
                e.undeclare();
            } catch (ZException ex) {
                throw new ZException("unregisterEval failed", ex);
            }
        }
    }

    private static Properties predicateToProperties(String predicate) {
        Properties result = new Properties();
        String[] kvs = predicate.split(";");
        for (String kv : kvs) {
            int i = kv.indexOf('=');
            if (i > 0) {
                result.setProperty(kv.substring(0, i), kv.substring(i + 1));
            }
        }
        return result;
    }

}
