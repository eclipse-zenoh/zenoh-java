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

import java.nio.charset.Charset;
import java.nio.ByteBuffer;
import java.util.Map;
import java.util.Properties;

/**
 * A {@link Value} containing {@link Properties}.
 */
public class PropertiesValue implements Value {

    private static final short ENCODING_FLAG = 0x03;

    private static final Charset utf8 = Charset.forName("UTF-8");

    private Properties p;

    /**
     * Creates a PropertiesValue containing some {@link Properties}.
     * 
     * @param p the properties
     */
    public PropertiesValue(Properties p) {
        this.p = p;
    }

    /**
     * Returns the properties from this PropertiesValue
     * 
     * @return the properties
     */
    public Properties getProperties() {
        return p;
    }

    @Override
    public Encoding getEncoding() {
        return Encoding.PROPERTIES;
    }

    @Override
    public ByteBuffer encode() {
        return ByteBuffer.wrap(toString().getBytes(utf8));
    }

    private static final String PROP_SEP = ";";
    private static final String KV_SEP = "=";

    @Override
    public String toString() {
        StringBuffer buf = new StringBuffer();
        int i = 0;
        for (Map.Entry<Object, Object> e : p.entrySet()) {
            buf.append(e.getKey()).append(KV_SEP).append(e.getValue());
            if (++i < p.size()) {
                buf.append(PROP_SEP);
            }
        }
        return buf.toString();
    }

    @Override
    public boolean equals(Object obj) {
        if (obj == null)
            return false;
        if (this == obj)
            return true;
        if (!(obj instanceof PropertiesValue))
            return false;

        return p.equals(((PropertiesValue) obj).p);
    }

    @Override
    public int hashCode() {
        return p.hashCode();
    }

    /**
     * The {@link Value.Decoder} for {@link PropertiesValue}s.
     */
    public static final Value.Decoder Decoder = new Value.Decoder() {

        @Override
        public short getEncodingFlag() {
            return ENCODING_FLAG;
        }

        private Properties fromString(String s) {
            Properties p = new Properties();
            if (s.length() > 0) {
                for (String kv : s.split(PROP_SEP)) {
                    int i = kv.indexOf(KV_SEP);
                    if (i < 0) {
                        p.setProperty(kv, "");
                    } else {
                        p.setProperty(kv.substring(0, i), kv.substring(i + 1));
                    }
                }
            }
            return p;
        }

        @Override
        public Value decode(ByteBuffer buf) {
            return new PropertiesValue(fromString(new String(buf.array(), utf8)));
        }
    };
}
