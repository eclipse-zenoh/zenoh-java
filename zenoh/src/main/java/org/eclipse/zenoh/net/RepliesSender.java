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

/**
 * Class to be used in a {@link QueryHandler} implementation to send back
 * replies to a query.
 */
public final class RepliesSender {

    private long send_replies_ptr;
    private long query_handle_ptr;

    private RepliesSender(long send_replies_ptr, long query_handle_ptr) {
        this.send_replies_ptr = send_replies_ptr;
        this.query_handle_ptr = query_handle_ptr;
    }

    /**
     * Send back the replies to the query associated with this {@link RepliesSender}
     * object.
     * 
     * @param replies the replies.
     */
    public void sendReplies(Resource[] replies) {
        org.eclipse.zenoh.swig.zenohc.call_replies_sender(send_replies_ptr, query_handle_ptr, replies);
    }
}
