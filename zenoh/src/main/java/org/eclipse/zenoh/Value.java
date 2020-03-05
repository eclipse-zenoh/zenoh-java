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

import java.nio.ByteBuffer;

/**
 * Interface of a Value that, associated to a {@link Path}, can be published
 * into zenoh via {@link Workspace#put(Path, Value)}, or retrieved via
 * {@link Workspace#get(Selector)} or via a subscription
 * ({@link Workspace#subscribe(Selector, Listener)}).
 */
public interface Value {

    /**
     * Interface of a Value decoder, able to transform a {@link ByteBuffer} into a
     * Value.
     */
    public interface Decoder {
        /**
         * Returns the flag of the {@link Encoding} that this Decoder supports. (@see
         * Encoding#getFlag()).
         * 
         * @return the encoding flag.
         */
        public short getEncodingFlag();

        /**
         * Decode a Value that is encoded in a {@link ByteBuffer}.
         * 
         * @param buf the ByteBuffer containing the encoded Value.
         * @return the Value.
         */
        public Value decode(ByteBuffer buf);
    }

    /**
     * Return the {@link Encoding} of this Value.
     * 
     * @return the encoding.
     */
    public Encoding getEncoding();

    /**
     * Returns a new {@link ByteBuffer} containing the Value encoded as a sequence
     * of bytes.
     * 
     * @return the encoded Value.
     */
    public ByteBuffer encode();
}
