package org.eclipse.zenoh;

import java.nio.ByteBuffer;
import java.nio.charset.Charset;

public class IntValue  implements Value {

    private static final short ENCODING_FLAG = 0x07;

    private static final Charset utf8 = Charset.forName("UTF-8");

    private int v;

    /**
     * Creates an IntValue containing a int.
     *
     * @param v the int
     */
    public IntValue(int v) {
        this.v = v;
    }

    /**
     * Returns the string from this StringValue
     *
     * @return the string
     */
    public int getInt() {
        return this.v;
    }

    @Override
    public Encoding getEncoding() {
        return Encoding.INT;
    }

    public static ByteBuffer encode(int v) {
        return ByteBuffer.wrap(Integer.toString(v).getBytes());
    }

    @Override
    public ByteBuffer encode() {
        return IntValue.encode(this.v);        
    }

    @Override
    public String toString() {
        return Integer.toString(this.v);
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

