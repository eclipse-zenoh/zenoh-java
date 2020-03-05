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
 * A callback interface to be implemented for the reception of replies for a
 * query. See {@link Session#query(String, String, ReplyHandler)} and
 * {@link Session#query(String, String, ReplyHandler, QueryDest, QueryDest)}.
 */
public interface ReplyHandler {

    /**
     * The method that will be called on reception of replies to the query sent by
     * {@link Session#query(String, String, ReplyHandler)} or
     * {@link Session#query(String, String, ReplyHandler, QueryDest, QueryDest)}.
     * 
     * @param reply is the actual reply.
     */
    public void handleReply(ReplyValue reply);

}
