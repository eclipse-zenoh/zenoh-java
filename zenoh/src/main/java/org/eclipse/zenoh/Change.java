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

import org.eclipse.zenoh.core.Timestamp;
import org.eclipse.zenoh.core.ZException;

/**
 * The notification of a change for a resource in zenoh. See {@link Listener}.
 */
public class Change {

    /**
     * The kind of Change: either {@link #PUT}, {@link #UPDATE} or {@link #REMOVE}.
     */
    public enum Kind {
        PUT(0x00), UPDATE(0x01), REMOVE(0x02);

        private int numVal;

        private Kind(int numVal) {
            this.numVal = numVal;
        }

        /**
         * Returns the numeric value of the change kind (the same than in zenoh-c).
         *
         * @return the numeric value.
         */
        public int value() {
            return numVal;
        }

        protected static Kind fromInt(int numVal) throws ZException {
            if (numVal == PUT.value()) {
                return PUT;
            } else if (numVal == UPDATE.value()) {
                return UPDATE;
            } else if (numVal == REMOVE.value()) {
                return REMOVE;
            } else {
                throw new ZException("Unkown change kind value: " + numVal);
            }
        }
    }

    private Path path;
    private Kind kind;
    private Timestamp timestamp;
    private Value value;

    protected Change(Path path, Kind kind, Timestamp timestamp, Value value) {
        this.path = path;
        this.kind = kind;
        this.timestamp = timestamp;
        this.value = value;
    }

    /**
     * Returns the {@link Path} of resource that changed.
     *
     * @return the resource path.
     */
    public Path getPath() {
        return path;
    }

    /**
     * Returns the {@link Kind} of change.
     *
     * @return the kind of change.
     */
    public Kind getKind() {
        return kind;
    }

    /**
     * Returns the {@link Timestamp} when change occured.
     *
     * @return the timestamp.
     */
    public Timestamp getTimestamp() {
        return timestamp;
    }

    /**
     * Depending of the change {@link Kind}, returns:
     * <ul>
     * <li>if kind is {@link Kind#PUT}: the new value</li>
     * <li>if kind is {@link Kind#UPDATE}: the delta value</li>
     * <li>if kind is {@link Kind#REMOVE}: null</li>
     * </ul>
     *
     * @return the new value (complete or delta), or null.
     */
    public Value getValue() {
        return value;
    }

}