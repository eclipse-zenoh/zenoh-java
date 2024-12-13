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

import com.google.common.reflect.TypeToken;
import io.zenoh.bytes.ZBytes;

import java.util.Arrays;
import java.util.List;
import java.util.Map;

import static io.zenoh.ext.ZDeserializerKt.zDeserialize;
import static io.zenoh.ext.ZSerializerKt.zSerialize;

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
        var zbytes1 = zSerialize(input1, new TypeToken<>() {});
        Boolean output1 = zDeserialize(zbytes1, new TypeToken<>() {});
        assert input1.equals(output1);

        // Byte example
        Byte input2 = 126;
        var zbytes2 = zSerialize(input2, new TypeToken<>() {});
        Byte output2 = zDeserialize(zbytes2, new TypeToken<>() {});
        assert input2.equals(output2);

        // Short example
        Short input3 = 256;
        var zbytes3 = zSerialize(input3, new TypeToken<>() {});
        Short output3 = zDeserialize(zbytes3, new TypeToken<>() {});
        assert input3.equals(output3);

        // Int example
        Integer input4 = 123456;
        var zbytes4 = zSerialize(input4, new TypeToken<>() {});
        Integer output4 = zDeserialize(zbytes4, new TypeToken<>() {});
        assert input4.equals(output4);

        // Long example
        Long input5 = 123456789L;
        var zbytes5 = zSerialize(input5, new TypeToken<>() {});
        Long output5 = zDeserialize(zbytes5, new TypeToken<>() {});
        assert input5.equals(output5);

        // Float example
        Float input6 = 123.45f;
        var zbytes6 = zSerialize(input6, new TypeToken<>() {});
        Float output6 = zDeserialize(zbytes6, new TypeToken<>() {});
        assert input6.equals(output6);

        // Double example
        Double input7 = 12345.6789;
        var zbytes7 = zSerialize(input7, new TypeToken<>() {});
        Double output7 = zDeserialize(zbytes7, new TypeToken<>() {});
        assert input7.equals(output7);

        // List example
        List<Integer> input12 = List.of(1, 2, 3, 4, 5);
        var zbytes12 = zSerialize(input12, new TypeToken<>() {});
        List<Integer> output12 = zDeserialize(zbytes12, new TypeToken<>() {});
        assert input12.equals(output12);

        // String example
        String input13 = "Hello, World!";
        var zbytes13 = zSerialize(input13, new TypeToken<>() {});
        String output13 = zDeserialize(zbytes13, new TypeToken<>() {});
        assert input13.equals(output13);

        // ByteArray example
        byte[] input14 = new byte[]{1, 2, 3, 4, 5};
        var zbytes14 = zSerialize(input14, new TypeToken<>() {});
        byte[] output14 = zDeserialize(zbytes14, new TypeToken<>() {});
        assert Arrays.equals(input14, output14);

        // Map example
        Map<String, Integer> input15 = Map.of("one", 1, "two", 2, "three", 3);
        var zbytes15 = zSerialize(input15, new TypeToken<>() {});
        Map<String, Integer> output15 = zDeserialize(zbytes15, new TypeToken<>() {
        });
        assert input15.equals(output15);

        // Nested List example
        List<List<Integer>> input18 = List.of(List.of(1, 2, 3));
        var zbytes18 = zSerialize(input18, new TypeToken<>() {});
        List<List<Integer>> output18 = zDeserialize(zbytes18, new TypeToken<>() {
        });
        assert input18.equals(output18);

        // Combined types example
        List<Map<String, Integer>> input19 = List.of(Map.of("a", 1, "b", 2));
        var zbytes19 = zSerialize(input19, new TypeToken<>() {});
        List<Map<String, Integer>> output19 = zDeserialize(zbytes19, new TypeToken<>() {
        });
        assert input19.equals(output19);
    }
}
