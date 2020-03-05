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
package org.eclipse.zenoh.net;

import org.scijava.nativelib.NativeLoader;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import org.eclipse.zenoh.core.ZException;
import org.eclipse.zenoh.swig.SWIGTYPE_p_zn_session_t;
import org.eclipse.zenoh.swig.result_kind;
import org.eclipse.zenoh.swig.zn_eval_p_result_t;
import org.eclipse.zenoh.swig.zn_pub_p_result_t;
import org.eclipse.zenoh.swig.zn_sto_p_result_t;
import org.eclipse.zenoh.swig.zn_sub_p_result_t;
import org.eclipse.zenoh.swig.zn_session_p_result_t;
import org.eclipse.zenoh.swig.zenohc;

import java.util.Map;
import java.util.Map.Entry;

/**
 * A zenoh-net session.
 */
public class Session {

    private static final Logger LOG = LoggerFactory.getLogger("org.eclipse.zenoh.net");
    private static final Map.Entry<Integer, byte[]>[] EMPTY = new Map.Entry[0];

    static {
        try {
            LOG.debug("Load native library: zenohc_java");
            NativeLoader.loadLibrary("zenohc_java");
        } catch (Exception e) {
            LOG.error("Failed to load native library zenohc_java: {}", e.toString());
            if (LOG.isDebugEnabled()) {
                e.printStackTrace();
            }
            System.exit(-1);
        }
    }

    private SWIGTYPE_p_zn_session_t s;

    private Session(SWIGTYPE_p_zn_session_t s) {
        this.s = s;
    }

    /**
     * Open a zenoh-net session.
     *
     * @param locator a string representing the network endpoint to which establish
     *                the session. A typical locator looks like this :
     *                <code>"tcp/127.0.0.1:7447"</code>. If <code>null</code>,
     *                open() will scout and try to establish the session
     *                automatically.
     * @return a Zenoh object representing the openned zenoh session..
     * @throws ZException if session etablishment fails.
     */
    public static Session open(String locator) throws ZException {
        return open(locator, null);
    }

    /**
     * Open a zenoh-net session.
     *
     * @param locator    a string representing the network endpoint to which
     *                   establish the session. A typical locator looks like this :
     *                   <code>"tcp/127.0.0.1:7447"</code>. If <code>null</code>,
     *                   open() will scout and try to establish the session
     *                   automatically.
     * @param properties a map of properties that will be used to establish and
     *                   configure the zenoh session. **properties** will typically
     *                   contain the <code>"username"</code> and
     *                   <code>"password"</code> informations needed to establish
     *                   the zenoh session with a secured infrastructure. It can be
     *                   set to <code>null</code>.
     * @return a Zenoh object representing the openned zenoh session..
     * @throws ZException if session etablishment fails.
     */
    public static Session open(String locator, Map<Integer, byte[]> properties) throws ZException {
        LOG.debug("Call zn_open on {}", locator);
        Entry<Integer, byte[]>[] entries = properties != null ? properties.entrySet().toArray(EMPTY) : null;
        zn_session_p_result_t session_result = zenohc.zn_open(locator, entries);
        if (session_result.getTag().equals(result_kind.Z_ERROR_TAG)) {
            throw new ZException("zn_open failed", session_result.getValue().getError());
        }
        SWIGTYPE_p_zn_session_t s = session_result.getValue().getSession();

        new Thread(new Runnable() {
            @Override
            public void run() {
                {
                    LOG.debug("Run zn_recv_loop");
                    zenohc.zn_recv_loop(s);
                }
            }
        }).start();

        return new Session(s);
    }

    /**
     * Close the zenoh-net session.
     * 
     * @throws ZException if close failed.
     */
    public void close() throws ZException {
        LOG.debug("Call zn_stop_recv_loop");
        int stop_result = zenohc.zn_stop_recv_loop(s);
        if (stop_result != 0) {
            throw new ZException("zn_stop_recv_loop failed", stop_result);
        }
        int close_result = zenohc.zn_close(s);
        if (close_result != 0) {
            throw new ZException("close_result failed", stop_result);
        }
    }

    /**
     * @return a map of properties containing various informations about the
     *         established zenoh-net session.
     */
    public Map<Integer, byte[]> info() {
        return zenohc.zn_info(s);
    }

    /**
     * Declare a publication for resource name <b>resource</b>.
     * 
     * @param resource the resource name to publish.
     * @return the zenoh {@link Publisher}.
     * @throws ZException if declaration fails.
     */
    public Publisher declarePublisher(String resource) throws ZException {
        LOG.debug("Call zn_declare_publisher for {}", resource);
        zn_pub_p_result_t pub_result = zenohc.zn_declare_publisher(s, resource);
        if (pub_result.getTag().equals(result_kind.Z_ERROR_TAG)) {
            throw new ZException("zn_declare_publisher on " + resource + " failed ", pub_result.getValue().getError());
        }
        return new Publisher(pub_result.getValue().getPub());
    }

    /**
     * Declare a subscribtion for all published data matching the provided resource
     * name <b>resource</b>.
     *
     * @param resource the resource name to subscribe to.
     * @param mode     the subscription mode.
     * @param handler  a {@link DataHandler} subclass implementing the callback
     *                 function that will be called each time a data matching the
     *                 subscribed resource name <b>resource</b> is received.
     * @return the zenoh-net {@link Subscriber}.
     * @throws ZException if declaration fails.
     */
    public Subscriber declareSubscriber(String resource, SubMode mode, DataHandler handler) throws ZException {
        LOG.debug("Call zn_declare_subscriber for {}", resource);
        zn_sub_p_result_t sub_result = zenohc.zn_declare_subscriber(s, resource, mode, handler);
        if (sub_result.getTag().equals(result_kind.Z_ERROR_TAG)) {
            throw new ZException("zn_declare_subscriber on " + resource + " failed ", sub_result.getValue().getError());
        }
        return new Subscriber(sub_result.getValue().getSub());
    }

    /**
     * Declare a storage for all data matching the provided resource name
     * <b>resource</b>.
     * 
     * @param resource the resource selection to store.
     * @param handler  a {@link StorageHandler} subclass implementing the callback
     *                 functions that will be called each time a data matching the
     *                 stored resource name <b>resource</b> is received and each
     *                 time a query for data matching the stored resource name
     *                 <b>resource</b> is received. The
     *                 {@link StorageHandler#handleQuery(String, String, RepliesSender)}
     *                 function MUST call the provided
     *                 {@link RepliesSender#sendReplies(Resource[])} function with
     *                 the resulting data.
     *                 {@link RepliesSender#sendReplies(Resource[])} can be called
     *                 with an empty array.
     * @return the zenoh {@link Storage}.
     * @throws ZException if declaration fails.
     */
    public Storage declareStorage(String resource, StorageHandler handler) throws ZException {
        LOG.debug("Call zn_declare_storage for {}", resource);
        zn_sto_p_result_t sto_result = zenohc.zn_declare_storage(s, resource, handler);
        if (sto_result.getTag().equals(result_kind.Z_ERROR_TAG)) {
            throw new ZException("zn_declare_storage on " + resource + " failed ", sto_result.getValue().getError());
        }
        return new Storage(sto_result.getValue().getSto());
    }

    /**
     * Declare an eval able to provide data matching the provided resource name
     * <b>resource</b>.
     * 
     * @param resource the resource to evaluate.
     * @param handler  a {@link QueryHandler} subclass implementing the the callback
     *                 function that will be called each time a query for data
     *                 matching the evaluated resource name <b>resource</b> is
     *                 received. The
     *                 {@link QueryHandler#handleQuery(String, String, RepliesSender)}
     *                 function MUST call the provided
     *                 {@link RepliesSender#sendReplies(Resource[])} function with
     *                 the resulting data.
     *                 {@link RepliesSender#sendReplies(Resource[])} can be called
     *                 with an empty array.
     * @return the Eval.
     * @throws ZException if declaration fails.
     */
    public Eval declareEval(String resource, QueryHandler handler) throws ZException {
        LOG.debug("Call zn_declare_eval for {}", resource);
        zn_eval_p_result_t eval_result = zenohc.zn_declare_eval(s, resource, handler);
        if (eval_result.getTag().equals(result_kind.Z_ERROR_TAG)) {
            throw new ZException("zn_declare_eval on " + resource + " failed ", eval_result.getValue().getError());
        }
        return new Eval(eval_result.getValue().getEval());
    }

    /**
     * Send data in a <i>write_data</i> message for the resource <b>resource</b>.
     * 
     * @param resource the resource name of the data to be sent.
     * @param payload  the data.
     * @throws ZException if write fails.
     */
    public void writeData(String resource, java.nio.ByteBuffer payload) throws ZException {
        int result = zenohc.zn_write_data(s, resource, payload);
        if (result != 0) {
            throw new ZException("zn_write_data of " + payload.capacity() + " bytes buffer on " + resource + "failed",
                    result);
        }
    }

    /**
     * Send data in a <i>write_data</i> message for the resource <b>resource</b>.
     * 
     * @param resource the resource name of the data to be sent.
     * @param payload  the data.
     * @param encoding a metadata information associated with the published data
     *                 that represents the encoding of the published data.
     * @param kind     a metadata information associated with the published data
     *                 that represents the kind of publication.
     * @throws ZException if write fails.
     */
    public void writeData(String resource, java.nio.ByteBuffer payload, short encoding, short kind) throws ZException {
        int result = zenohc.zn_write_data_wo(s, resource, payload, encoding, kind);
        if (result != 0) {
            throw new ZException(
                    "zn_write_data_wo of " + payload.capacity() + " bytes buffer on " + resource + "failed", result);
        }
    }

    /**
     * Query data matching resource name <b>resource</b>.
     * 
     * @param resource  the resource to query.
     * @param predicate a string that will be propagated to the storages and evals
     *                  that should provide the queried data. It may allow them to
     *                  filter, transform and/or compute the queried data. .
     * @param handler   a {@link ReplyHandler} subclass implementing the callback
     *                  function that will be called on reception of the replies of
     *                  the query.
     * @throws ZException if fails.
     */
    public void query(String resource, String predicate, ReplyHandler handler) throws ZException {
        query(resource, predicate, handler, QueryDest.bestMatch(), QueryDest.bestMatch());
    }

    /**
     * Query data matching resource name <b>resource</b>.
     * 
     * @param resource      the resource to query.
     * @param predicate     a string that will be propagated to the storages and
     *                      evals that should provide the queried data. It may allow
     *                      them to filter, transform and/or compute the queried
     *                      data. .
     * @param handler       a {@link ReplyHandler} subclass implementing the
     *                      callback function that will be called on reception of
     *                      the replies of the query.
     * @param dest_storages a {@link QueryDest} indicating which matching storages
     *                      should be destination of the query.
     * @param dest_evals    a {@link QueryDest} indicating which matching evals
     *                      should be destination of the query.
     * @throws ZException if fails.
     */
    public void query(String resource, String predicate, ReplyHandler handler, QueryDest dest_storages,
            QueryDest dest_evals) throws ZException {
        int result = zenohc.zn_query_wo(s, resource, predicate, handler, dest_storages, dest_evals);
        if (result != 0) {
            throw new ZException("zn_query on " + resource + " failed", result);
        }
    }

    protected static void LogException(Throwable e, String message) {
        LOG.warn(message, e);
    }

}
