//
// Copyright (c) 2023 ZettaScale Technology
//
// This program and the accompanying materials are made available under the
// terms of the Eclipse Public License 2.0 which is available at
// http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
// which is available at https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
//
// Contributors:
//   ZettaScale Zenoh Team, <zenoh@zettascale.tech>
//

package io.zenoh;

import io.zenoh.bytes.ZBytes;
import io.zenoh.ext.ZDeserializer;
import io.zenoh.ext.ZSerializer;

import java.util.Arrays;
import java.util.List;
import java.util.Map;

public class ZBytesExamples {

    public static void main(String[] args) {

        /*
         * ZBytes
         *
         * A ZBytes instance can be created from a String and from a Byte Array with the `ZBytes.from(string: String)`
         * and `ZBytes.from(bytes: byte[])` functions.
         *
         * A ZBytes can be converted back into a [String] with the functions [ZBytes.toString] and [ZBytes.tryToString].
         * Similarly, with [ZBytes.toBytes] you can get the inner byte representation.
         */

        var exampleString = "example string";
        var zbytesA = ZBytes.from(exampleString);
        var outputA = zbytesA.toString();
        assert exampleString.equals(outputA);

        var exampleBytes = new byte[]{1, 2, 3, 4, 5};
        var zbytesB = ZBytes.from(exampleBytes);
        var outputB = zbytesB.toBytes();
        assert Arrays.equals(exampleBytes, outputB);

        /*
         * Serialization and deserialization.
         *
         * Additionally, the Zenoh API provides a series of serialization and deserialization utilities for processing
         * the received payloads.
         *
         * Serialization and deserialization supports the following types:
         * - Boolean
         * - Byte
         * - Byte Array
         * - Short
         * - Int
         * - Long
         * - Float
         * - Double
         * - String
         * - List
         * - Map
         *
         * For List and Map, the inner types can be a combination of the above types, including themselves.
         *
         * These serialization and deserialization utilities can be used across the Zenoh ecosystem with Zenoh
         * versions based on other supported languages such as Rust, Python, C and C++.
         * This works when the types are equivalent (a `Byte` corresponds to an `i8` in Rust, a `Short` to an `i16`, etc).
         */

        // Boolean example
        Boolean input1 = true;
        ZSerializer<Boolean> serializer1 = new ZSerializer<>() {};
        ZBytes zbytes1 = serializer1.serialize(input1);

        ZDeserializer<Boolean> deserializer1 = new ZDeserializer<>() {};
        Boolean output1 = deserializer1.deserialize(zbytes1);
        assert input1.equals(output1);

        // Byte example
        Byte input2 = 126;
        ZSerializer<Byte> serializer2 = new ZSerializer<>() {};
        ZBytes zbytes2 = serializer2.serialize(input2);

        ZDeserializer<Byte> deserializer2 = new ZDeserializer<>() {};
        Byte output2 = deserializer2.deserialize(zbytes2);
        assert input2.equals(output2);

        // Short example
        Short input3 = 256;
        ZSerializer<Short> serializer3 = new ZSerializer<>() {};
        ZBytes zbytes3 = serializer3.serialize(input3);

        ZDeserializer<Short> deserializer3 = new ZDeserializer<>() {};
        Short output3 = deserializer3.deserialize(zbytes3);
        assert input3.equals(output3);

        // Int example
        Integer input4 = 123456;
        ZSerializer<Integer> serializer4 = new ZSerializer<>() {};
        ZBytes zbytes4 = serializer4.serialize(input4);

        ZDeserializer<Integer> deserializer4 = new ZDeserializer<>() {};
        Integer output4 = deserializer4.deserialize(zbytes4);
        assert input4.equals(output4);

        // Long example
        Long input5 = 123456789L;
        ZSerializer<Long> serializer5 = new ZSerializer<>() {};
        ZBytes zbytes5 = serializer5.serialize(input5);

        ZDeserializer<Long> deserializer5 = new ZDeserializer<>() {};
        Long output5 = deserializer5.deserialize(zbytes5);
        assert input5.equals(output5);

        // Float example
        Float input6 = 123.45f;
        ZSerializer<Float> serializer6 = new ZSerializer<>() {};
        ZBytes zbytes6 = serializer6.serialize(input6);

        ZDeserializer<Float> deserializer6 = new ZDeserializer<>() {};
        Float output6 = deserializer6.deserialize(zbytes6);
        assert input6.equals(output6);

        // Double example
        Double input7 = 12345.6789;
        ZSerializer<Double> serializer7 = new ZSerializer<>() {};
        ZBytes zbytes7 = serializer7.serialize(input7);

        ZDeserializer<Double> deserializer7 = new ZDeserializer<>() {};
        Double output7 = deserializer7.deserialize(zbytes7);
        assert input7.equals(output7);

        // List example
        List<Integer> input12 = List.of(1, 2, 3, 4, 5);
        ZSerializer<List<Integer>> serializer12 = new ZSerializer<>() {};
        ZBytes zbytes12 = serializer12.serialize(input12);

        ZDeserializer<List<Integer>> deserializer12 = new ZDeserializer<>() {};
        List<Integer> output12 = deserializer12.deserialize(zbytes12);
        assert input12.equals(output12);

        // String example
        String input13 = "Hello, World!";
        ZSerializer<String> serializer13 = new ZSerializer<>() {};
        ZBytes zbytes13 = serializer13.serialize(input13);

        ZDeserializer<String> deserializer13 = new ZDeserializer<>() {};
        String output13 = deserializer13.deserialize(zbytes13);
        assert input13.equals(output13);

        // ByteArray example
        byte[] input14 = new byte[]{1, 2, 3, 4, 5};
        ZSerializer<byte[]> serializer14 = new ZSerializer<>() {};
        ZBytes zbytes14 = serializer14.serialize(input14);

        ZDeserializer<byte[]> deserializer14 = new ZDeserializer<>() {};
        byte[] output14 = deserializer14.deserialize(zbytes14);
        assert Arrays.equals(input14, output14);

        // Map example
        Map<String, Integer> input15 = Map.of("one", 1, "two", 2, "three", 3);
        ZSerializer<Map<String, Integer>> serializer15 = new ZSerializer<>() {};
        ZBytes zbytes15 = serializer15.serialize(input15);

        ZDeserializer<Map<String, Integer>> deserializer15 = new ZDeserializer<>() {};
        Map<String, Integer> output15 = deserializer15.deserialize(zbytes15);
        assert input15.equals(output15);

        // Nested List example
        List<List<Integer>> input18 = List.of(List.of(1, 2, 3));
        ZSerializer<List<List<Integer>>> serializer18 = new ZSerializer<>() {};
        ZBytes zbytes18 = serializer18.serialize(input18);

        ZDeserializer<List<List<Integer>>> deserializer18 = new ZDeserializer<>() {};
        List<List<Integer>> output18 = deserializer18.deserialize(zbytes18);
        assert input18.equals(output18);

        // Combined types example
        List<Map<String, Integer>> input19 = List.of(Map.of("a", 1, "b", 2));
        ZSerializer<List<Map<String, Integer>>> serializer19 = new ZSerializer<>() {};
        ZBytes zbytes19 = serializer19.serialize(input19);

        ZDeserializer<List<Map<String, Integer>>> deserializer19 = new ZDeserializer<>() {};
        List<Map<String, Integer>> output19 = deserializer19.deserialize(zbytes19);
        assert input19.equals(output19);
    }
}
