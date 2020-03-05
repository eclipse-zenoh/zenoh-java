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
 * A {@link Value} containing a {@link ByteBuffer}.
 */
public class RawValue implements Value {

    private static final short ENCODING_FLAG = 0x00;

    private ByteBuffer buf;

    /**
     * Creates a RawValue containing a {@link ByteBuffer}.
     * 
     * @param buf the bytes buffer.
     */
    public RawValue(ByteBuffer buf) {
        this.buf = buf;
    }

    /**
     * Returns the {@link ByteBuffer} from this RawValue
     * 
     * @return the bytes buffer.
     */
    public ByteBuffer getBuffer() {
        return buf;
    }

    @Override
    public Encoding getEncoding() {
        return Encoding.RAW;
    }

    @Override
    public ByteBuffer encode() {
        return buf;
    }

    @Override
    public String toString() {
        return buf.toString();
    }

    @Override
    public boolean equals(Object obj) {
        if (obj == null)
            return false;
        if (this == obj)
            return true;
        if (!(obj instanceof RawValue))
            return false;

        return buf.equals(((RawValue) obj).buf);
    }

    @Override
    public int hashCode() {
        return buf.hashCode();
    }

    /**
     * The {@link Value.Decoder} for {@link RawValue}s.
     */
    public static final Value.Decoder Decoder = new Value.Decoder() {
        @Override
        public short getEncodingFlag() {
            return ENCODING_FLAG;
        }

        @Override
        public Value decode(ByteBuffer buf) {
            return new RawValue(buf);
        }
    };
}
