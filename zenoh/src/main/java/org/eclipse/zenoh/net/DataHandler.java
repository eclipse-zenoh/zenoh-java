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

/**
 * A callback interface to be implemented for the reception of subscribed/stored
 * resources. See
 * {@link Session#declareSubscriber(String, SubMode, DataHandler)} and
 * {@link Session#declareStorage(String, StorageHandler)}.
 */
public interface DataHandler {

    /**
     * The method that will be called on reception of data matching the subscribed
     * or stored resource.
     *
     * @param rname the resource name of the received data.
     * @param data  the received data.
     * @param info  the {@link DataInfo} associated with the received data.
     */
    public void handleData(String rname, ByteBuffer data, DataInfo info);

}
