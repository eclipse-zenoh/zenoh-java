package org.eclipse.zenoh;

import java.nio.ByteBuffer;
import java.nio.charset.Charset;

/**
 * A {@link Value} containing a 64-bits signed integer (i.e. a Java long).
 */
public class IntValue  implements Value {

    private static final short ENCODING_FLAG = 0x06;

    private static final Charset utf8 = Charset.forName("UTF-8");

    private long v;

    /**
     * Creates an IntValue containing a long.
     *
     * @param v the long
     */
    public IntValue(long v) {
        this.v = v;
    }

    /**
     * Returns the string from this StringValue
     *
     * @return the string
     */
    public long getInt() {
        return this.v;
    }

    @Override
    public Encoding getEncoding() {
        return Encoding.INT;
    }

    public static ByteBuffer encode(long v) {
        return ByteBuffer.wrap(Long.toString(v).getBytes());
    }

    @Override
    public ByteBuffer encode() {
        return IntValue.encode(this.v);        
    }

    @Override
    public String toString() {
        return Long.toString(this.v);
    }

    @Override
    public boolean equals(Object obj) {
        if (obj == null)
            return false;
        if (this == obj)
            return true;
        if (!(obj instanceof IntValue))
            return false;

        return this.v == (((IntValue) obj).v);
    }

    @Override
    public int hashCode() {
        return this.hashCode();
    }

    /**
     * The {@link Value.Decoder} for {@link StringValue}s.
     */
    public static final Value.Decoder Decoder = new Value.Decoder() {
        @Override
        public short getEncodingFlag() {
            return ENCODING_FLAG;
        }

        @Override
        public Value decode(ByteBuffer buf) {
            return new IntValue(Integer.parseInt(new String(buf.array(), utf8)));
        }
    };
}

