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

import java.nio.ByteBuffer;

import org.eclipse.zenoh.core.ZException;
import org.eclipse.zenoh.swig.zn_pub_t;
import org.eclipse.zenoh.swig.zenohc;

/**
 * A Publisher (see {@link Session#declarePublisher(String)}).
 */
public class Publisher {

    private zn_pub_t pub;

    protected Publisher(zn_pub_t pub) {
        this.pub = pub;
    }

    /**
     * Undeclare the Publisher.
     * 
     * @throws ZException if undeclaration failed.
     */
    public void undeclare() throws ZException {
        int error = zenohc.zn_undeclare_publisher(pub);
        if (error != 0) {
            throw new ZException("zn_undeclare_publisher failed ", error);
        }
    }

    /**
     * Send data in a <i>compact_data</i> message for the resource published by this
     * Publisher.
     * 
     * @param data the data to be sent.
     * @throws ZException if write fails.
     */
    public void streamCompactData(ByteBuffer data) throws ZException {
        int result = zenohc.zn_stream_compact_data(pub, data);
        if (result != 0) {
            throw new ZException("zn_stream_compact_data of " + data.capacity() + " bytes buffer failed", result);
        }
    }

    /**
     * Send data in a <i>stream_data</i> message for the resource published by this
     * Publisher.
     * 
     * @param data the data to be sent.
     * @throws ZException if write fails.
     */
    public void streamData(ByteBuffer data) throws ZException {
        int result = zenohc.zn_stream_data(pub, data);
        if (result != 0) {
            throw new ZException("zn_stream_data of " + data.capacity() + " bytes buffer failed", result);
        }
    }

    /**
     * Send data in a <i>stream_data</i> message for the resource published by this
     * Publisher.
     * 
     * @param data     the data to be sent.
     * @param encoding a metadata information associated with the published data
     *                 that represents the encoding of the published data.
     * @param kind     a metadata information associated with the published data
     *                 that represents the kind of publication.
     * @throws ZException if write fails.
     */
    public void streamData(ByteBuffer data, short encoding, short kind) throws ZException {
        int result = zenohc.zn_stream_data_wo(pub, data, encoding, kind);
        if (result != 0) {
            throw new ZException("zn_stream_data of " + data.capacity() + " bytes buffer failed", result);
        }
    }

}