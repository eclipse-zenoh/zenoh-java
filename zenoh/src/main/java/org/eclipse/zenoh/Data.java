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
package org.eclipse.zenoh;

import org.slf4j.LoggerFactory;

import org.eclipse.zenoh.core.Timestamp;

/**
 * A zenoh data returned by a {@link Workspace#get(Selector)} query. The Data
 * objects are comparable according to their {@link Timestamp}. Note that zenoh
 * makes sure that each published path/value has a unique timestamp accross the
 * system.
 */
public class Data implements Comparable<Data> {

    private Path path;
    private Value value;
    private Timestamp timestamp;

    protected Data(Path path, Value value, Timestamp timestamp) {
        this.path = path;
        this.value = value;
        this.timestamp = timestamp;
    }

    /**
     * Returns the {@link Path} of the data.
     *
     * @return the path of the data.
     */
    public Path getPath() {
        return path;
    }

    /**
     * Returns the {@link Value} of the data.
     *
     * @return the value of the data.
     */
    public Value getValue() {
        return value;
    }

    /**
     * Returns the {@link Timestamp} of the data.
     *
     * @return the timestamp of the data.
     */
    public Timestamp getTimestamp() {
        return timestamp;
    }

    @Override
    public int compareTo(Data o) {
        // Order entires according to their timestamps
        return timestamp.compareTo(o.timestamp);
    }

    @Override
    public boolean equals(Object obj) {
        if (obj == null)
            return false;
        if (this == obj)
            return true;
        if (!(obj instanceof Data))
            return false;

        Data o = (Data) obj;
        // As timestamp is unique per data, only compare timestamps.
        if (!timestamp.equals(o.timestamp))
            return false;

        // however, log a warning if path and values are different...
        if (!path.equals(o.path)) {
            LoggerFactory.getLogger("org.eclipse.zenoh").warn(
                    "INTERNAL ERROR: 2 entries with same timestamp {} have different paths: {} vs. {}", timestamp, path,
                    o.path);
        }
        if (!value.equals(o.value)) {
            LoggerFactory.getLogger("org.eclipse.zenoh").warn(
                    "INTERNAL ERROR: 2 entries with same timestamp {} have different values: {} vs. {}", timestamp,
                    value, o.value);
        }

        return true;
    }

    @Override
    public int hashCode() {
        return timestamp.hashCode();
    }
}