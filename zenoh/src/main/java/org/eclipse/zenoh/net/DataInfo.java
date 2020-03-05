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

import org.eclipse.zenoh.core.Timestamp;

/**
 * Data structure containing meta informations about the associated data.
 */
public class DataInfo {

    private Timestamp tstamp = null;
    private int encoding = 0;
    private int kind = 0;

    private static final long ZN_T_STAMP = 0x10;
    private static final long ZN_KIND = 0x20;
    private static final long ZN_ENCODING = 0x40;

    protected DataInfo(long flags, Timestamp tstamp, int encoding, int kind) {
        if ((flags & ZN_T_STAMP) != 0L) {
            this.tstamp = tstamp;
        }
        if ((flags & ZN_KIND) != 0L) {
            this.kind = kind;
        }
        if ((flags & ZN_ENCODING) != 0L) {
            this.encoding = encoding;
        }
    }

    /**
     * @return the encoding of the data.
     */
    public int getEncoding() {
        return encoding;
    }

    /**
     * @return the unique timestamp at which the data has been produced.
     */
    public Timestamp getTimestamp() {
        return tstamp;
    }

    /**
     * @return the kind of the data.
     */
    public int getKind() {
        return kind;
    }
}
