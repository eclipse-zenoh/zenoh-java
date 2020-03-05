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

import org.eclipse.zenoh.core.ZException;

/**
 * A description of the {@link Value} format, allowing zenoh to know how to
 * encode/decode the value to/from a bytes buffer.
 */
public enum Encoding {

    /**
     * The value has a RAW encoding (i.e. it's a bytes buffer).
     */
    RAW(RawValue.Decoder.getEncodingFlag(), RawValue.Decoder),
    /**
     * The value is an UTF-8 string.
     */
    STRING(StringValue.Decoder.getEncodingFlag(), StringValue.Decoder),
    /**
     * The value if a list of keys/values, encoded as an UTF-8 string. The
     * keys/values are separated by ';' character, and each key is separated from
     * its associated value (if any) with a '=' character.
     */
    PROPERTIES(PropertiesValue.Decoder.getEncodingFlag(), PropertiesValue.Decoder),
    /**
     * The value is a JSON structure in an UTF-8 string.
     */
    JSON((short) 0x04, StringValue.Decoder);

    private short flag;
    private Value.Decoder decoder;

    private Encoding(short flag, Value.Decoder decoder) {
        this.flag = flag;
        this.decoder = decoder;
    }

    /**
     * Returns the numeric flag corresponding to the encoding, for usage with
     * zenoh-net.
     *
     * @return the encoding flag.
     */
    public short getFlag() {
        return flag;
    }

    /**
     * Returns the {@link Value.Decoder} for this encoding.
     *
     * @return the decoder.
     */
    public Value.Decoder getDecoder() {
        return decoder;
    }

    protected static Encoding fromFlag(short flag) throws ZException {
        if (flag == RAW.getFlag()) {
            return RAW;
        } else if (flag == STRING.getFlag()) {
            return STRING;
        } else if (flag == PROPERTIES.getFlag()) {
            return PROPERTIES;
        } else if (flag == JSON.getFlag()) {
            return JSON;
        } else {
            throw new ZException("Unkown encoding flag: " + flag);
        }
    }

}
