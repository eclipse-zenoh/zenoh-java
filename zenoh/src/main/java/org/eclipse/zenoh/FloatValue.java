package org.eclipse.zenoh;

import java.nio.ByteBuffer;
import java.nio.charset.Charset;

/**
 * A {@link Value} containing a 64-bits signed float (i.e. a Java double).
 */
public class FloatValue  implements Value {

    private static final short ENCODING_FLAG = 0x07;

    private static final Charset utf8 = Charset.forName("UTF-8");

    private double v;

    /**
     * Creates an IntValue containing a int.
     *
     * @param v the float
     */
    public FloatValue(double v) {
        this.v = v;
    }

    /**
     * Returns the string from this StringValue
     *
     * @return the string
     */
    public double getFloat() {
        return this.v;
    }

    @Override
    public Encoding getEncoding() {
        return Encoding.FLOAT;
    }

    public static ByteBuffer encode(double v) {
        return ByteBuffer.wrap(Double.toString(v).getBytes());
    }

    @Override
    public ByteBuffer encode() {
        return FloatValue.encode(this.v);
    }

    @Override
    public String toString() {
        return Double.toString(this.v);
    }

    @Override
    public boolean equals(Object obj) {
        if (obj == null)
            return false;
        if (this == obj)
            return true;
        if (!(obj instanceof FloatValue))
            return false;

        return this.v == (((FloatValue) obj).v);
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
            return new FloatValue(Float.parseFloat(new String(buf.array(), utf8)));
        }
    };
}


