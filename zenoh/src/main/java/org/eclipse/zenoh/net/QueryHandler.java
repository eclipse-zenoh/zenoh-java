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
 * A callback interface to be implemented for handling of queries on storages or
 * evals. See {@link Session#declareStorage(String, StorageHandler)} and
 * {@link Session#declareEval(String, QueryHandler)}.
 */
public interface QueryHandler {

    /**
     * The method that will be called on reception of query matching the
     * stored/evaluated resource selection. The implementation must provide the data
     * matching the resource <i>rname</i> by calling the
     * {@link RepliesSender#sendReplies(Resource[])} method with the data as
     * argument. The {@link RepliesSender#sendReplies(Resource[])} method MUST be
     * called but accepts empty data array. This call can be made in the current
     * Thread or in a different Thread.
     * 
     * @param rname         the resource name of the queried data.
     * @param predicate     a string provided by the querier refining the data to be
     *                      provided.
     * @param repliesSender a {@link RepliesSender} on which the
     *                      <i>sendReplies()</i> function MUST be called with the
     *                      provided data as argument.
     */
    public void handleQuery(String rname, String predicate, RepliesSender repliesSender);

}