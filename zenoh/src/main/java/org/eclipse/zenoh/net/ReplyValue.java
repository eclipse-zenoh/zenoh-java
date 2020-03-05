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

import org.eclipse.zenoh.core.ZException;
import java.nio.ByteBuffer;

/**
 * A data structure containing one of the replies to a query (see
 * {@link ReplyHandler#handleReply(ReplyValue)}).
 */
public class ReplyValue {

    /**
     * The reply message kind.
     */
    public enum Kind {

        /**
         * The reply contains some data from a storage.
         */
        ZN_STORAGE_DATA(0),

        /**
         * The reply indicates that no more data is expected from the specified storage.
         */
        ZN_STORAGE_FINAL(1),

        /**
         * The reply contains some data from an eval.
         */
        ZN_EVAL_DATA(2),

        /**
         * The reply indicates that no more data is expected from the specified eval.
         */
        ZN_EVAL_FINAL(3),

        /**
         * The reply indicates that no more replies are expected for the query.
         */
        ZN_REPLY_FINAL(4);

        private int numVal;

        Kind(int numVal) {
            this.numVal = numVal;
        }

        public static Kind fromInt(int numVal) throws ZException {
            if (numVal == ZN_STORAGE_DATA.value()) {
                return ZN_STORAGE_DATA;
            } else if (numVal == ZN_STORAGE_FINAL.value()) {
                return ZN_STORAGE_FINAL;
            }
            if (numVal == ZN_EVAL_DATA.value()) {
                return ZN_EVAL_DATA;
            } else if (numVal == ZN_EVAL_FINAL.value()) {
                return ZN_EVAL_FINAL;
            } else if (numVal == ZN_REPLY_FINAL.value()) {
                return ZN_REPLY_FINAL;
            } else {
                throw new ZException("INTERNAL ERROR: cannot create ReplyValue.Kind from int: " + numVal);
            }
        }

        public int value() {
            return numVal;
        }
    }

    private Kind kind;
    private byte[] srcid;
    private long rsn;
    private String rname;
    private ByteBuffer data;
    private DataInfo info;

    protected ReplyValue(int kind, byte[] srcid, long rsn, String rname, ByteBuffer data, DataInfo info)
            throws ZException {
        this(Kind.fromInt(kind), srcid, rsn, rname, data, info);
    }

    protected ReplyValue(Kind kind, byte[] srcid, long rsn, String rname, ByteBuffer data, DataInfo info) {
        this.kind = kind;
        this.srcid = srcid;
        this.rsn = rsn;
        this.rname = rname;
        this.data = data;
        this.info = info;
    }

    /**
     * @return the Reply message kind.
     */
    public Kind getKind() {
        return kind;
    }

    /**
     * @return the unique identifier of the storage or eval that sent this reply
     *         when {@link ReplyValue#kind} equals {@link Kind#ZN_STORAGE_DATA},
     *         {@link Kind#ZN_STORAGE_FINAL}, {@link Kind#ZN_EVAL_DATA} or
     *         {@link Kind#ZN_EVAL_FINAL}.
     */
    public byte[] getSrcId() {
        return srcid;
    }

    /**
     * @return the sequence number of the reply from the identified storage or eval
     *         when {@link ReplyValue#kind} equals {@link Kind#ZN_STORAGE_DATA},
     *         {@link Kind#ZN_STORAGE_FINAL}, {@link Kind#ZN_EVAL_DATA} or
     *         {@link Kind#ZN_EVAL_FINAL}.
     */
    public long getRsn() {
        return rsn;
    }

    /**
     * @return the resource name of the received data when {@link ReplyValue#kind}
     *         equals {@link Kind#ZN_STORAGE_DATA} or {@link Kind#ZN_EVAL_DATA}.
     */
    public String getRname() {
        return rname;
    }

    /**
     * @return the received data when {@link ReplyValue#kind} equals
     *         {@link Kind#ZN_STORAGE_DATA} or {@link Kind#ZN_EVAL_DATA}.
     */
    public ByteBuffer getData() {
        return data;
    }

    /**
     * @return some meta information about the received data when
     *         {@link ReplyValue#kind} equals {@link Kind#ZN_STORAGE_DATA} or
     *         {@link Kind#ZN_EVAL_DATA}.
     */
    public DataInfo getInfo() {
        return info;
    }
}
