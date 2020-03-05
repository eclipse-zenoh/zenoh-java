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
import org.eclipse.zenoh.swig.zenohc;
import org.eclipse.zenoh.swig.zn_sub_t;

/**
 * A Subscriber (see
 * {@link Session#declareSubscriber(String, SubMode, SubscriberCallback)}).
 */
public class Subscriber {

    private zn_sub_t sub;

    protected Subscriber(zn_sub_t sub) {
        this.sub = sub;
    }

    /**
     * Pull data for the {@link SubMode.Kind#ZN_PULL_MODE} or
     * {@link SubMode.Kind#ZN_PERIODIC_PULL_MODE} subscribtion. The pulled data will
     * be provided by calling the
     * {@link DataHandler#handleData(String, java.nio.ByteBuffer, DataInfo)}
     * function provided to the
     * {@link Session#declareSubscriber(String, SubMode, DataHandler)} function.
     * 
     * @throws ZException if pull failed.
     */
    public void pull() throws ZException {
        int result = zenohc.zn_pull(sub);
        if (result != 0) {
            throw new ZException("zn_pull failed", result);
        }
    }

    /**
     * Undeclare the Subscriber.
     * 
     * @throws ZException if undeclaration failed.
     */
    public void undeclare() throws ZException {
        int error = zenohc.zn_undeclare_subscriber(sub);
        if (error != 0) {
            throw new ZException("zn_undeclare_subscriber failed ", error);
        }
    }

}