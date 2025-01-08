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

package io.zenoh.ext

import com.google.common.reflect.TypeToken
import io.zenoh.bytes.IntoZBytes
import io.zenoh.bytes.ZBytes
import io.zenoh.jni.JNIZBytes

/**
 * Zenoh deserializer.
 *
 * This class is a utility for deserializing [ZBytes] into elements of type [T].
 *
 * This class supports the following types:
 * - [Boolean]
 * - [Byte]
 * - [Short]
 * - [Int]
 * - [Long]
 * - [Float]
 * - [Double]
 * - [List]
 * - [String]
 * - [ByteArray]
 * - [Map]
 *
 * For List and Map, the inner types can be a combination of the above types, including themselves.
 *
 * Due to Java's type erasure, an actual implementation of this abstract class needs to be created (see the examples below).
 *
 * This deserialization utility can be used across the Zenoh ecosystem with Zenoh
 * versions based on other supported languages such as Rust, Python, C and C++.
 * This works when the types are equivalent (a `Byte` corresponds to an `i8` in Rust, a `Short` to an `i16`, etc).
 *
 * # Examples
 *
 * Example for a basic type, in this case an integer:
 * ```java
 * Integer input = 123456;
 * ZSerializer<Integer> serializer = new ZSerializer<>() {};
 * ZBytes zbytes = serializer.serialize(input);
 *
 * ZDeserializer<Integer> deserializer = new ZDeserializer<>() {};
 * Integer output = deserializer.deserialize(zbytes);
 * assert input.equals(output);
 * ```
 *
 * Examples for parameterized types:
 * - List
 * ```java
 * List<Integer> input = List.of(1, 2, 3, 4, 5);
 * ZSerializer<List<Integer>> serializer = new ZSerializer<>() {};
 * ZBytes zbytes = serializer.serialize(input12);
 *
 * ZDeserializer<List<Integer>> deserializer = new ZDeserializer<>() {};
 * List<Integer> output = deserializer.deserialize(zbytes);
 * assert input.equals(output);
 * ```
 *
 * - Map
 * ```java
 * Map<String, Integer> input = Map.of("one", 1, "two", 2, "three", 3);
 * ZSerializer<Map<String, Integer>> serializer = new ZSerializer<>() {};
 * ZBytes zbytes = serializer.serialize(input);
 *
 * ZDeserializer<Map<String, Integer>> deserializer = new ZDeserializer<>() {};
 * Map<String, Integer> output = deserializer.deserialize(zbytes);
 * assert input.equals(output);
 * ```
 *
 * As mentioned, for List and Map, the inner types can be a combination of the above types, including themselves.
 * Here's an example with a List of Maps:
 * ```java
 * List<Map<String, Integer>> input = List.of(Map.of("a", 1, "b", 2));
 * ZSerializer<List<Map<String, Integer>>> serializer = new ZSerializer<>() {};
 * ZBytes zbytes = serializer.serialize(input);
 *
 * ZDeserializer<List<Map<String, Integer>>> deserializer = new ZDeserializer<>() {};
 * List<Map<String, Integer>> output = deserializer.deserialize(zbytes);
 * assert input.equals(output);
 * ```
 *
 * For more examples, see the ZBytesExamples in the examples.
 *
 * @param T The deserialization type.
 * @see ZBytes
 * @see ZSerializer
 */
abstract class ZDeserializer<T>: TypeToken<T>() {

    /**
     * Deserialize the [zbytes] into an element of type [T].
     */
    fun deserialize(zbytes: IntoZBytes): T {
        @Suppress("UNCHECKED_CAST")
        return JNIZBytes.deserializeViaJNI(zbytes.into(), this.type) as T
    }
}
