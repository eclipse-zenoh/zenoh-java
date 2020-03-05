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
import org.eclipse.zenoh.swig.zn_sto_t;

/**
 * A Storage (see {@link Session#declareStorage(String, StorageCallback)}).
 */
public class Storage {

    private zn_sto_t sto;

    protected Storage(zn_sto_t sto) {
        this.sto = sto;
    }

    /**
     * Undeclare the Storage.
     * 
     * @throws ZException if undeclaration failed.
     */
    public void undeclare() throws ZException {
        int error = zenohc.zn_undeclare_storage(sto);
        if (error != 0) {
            throw new ZException("zn_undeclare_storage failed ", error);
        }
    }

}