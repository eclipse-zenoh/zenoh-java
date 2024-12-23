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

import io.zenoh.ext.ZDeserializer;
import io.zenoh.ext.ZSerializer;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.junit.runners.JUnit4;

import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.stream.Collectors;
import java.util.stream.Stream;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;

@RunWith(JUnit4.class)
public class ZBytesTests {

    /***********************************************
     * Standard serialization and deserialization. *
     ***********************************************/

    @Test
    public void testIntSerializationAndDeserialization() {
        int intInput = 1234;
        var serializer = new ZSerializer<Integer>() {};
        var payload = serializer.serialize(intInput);

        var deserializer = new ZDeserializer<Integer>() {};
        int intOutput = deserializer.deserialize(payload);
        assertEquals(intInput, intOutput);
    }

    @Test
    public void testFloatSerializationAndDeserialization() {
        float floatInput = 3.1415f;

        ZSerializer<Float> serializer = new ZSerializer<>() {};
        var payload = serializer.serialize(floatInput);

        ZDeserializer<Float> deserializer = new ZDeserializer<>() {};
        float floatOutput = deserializer.deserialize(payload);

        assertEquals(floatInput, floatOutput, 0.0001);
    }

    @Test
    public void testStringSerializationAndDeserialization() {
        String stringInput = "example";

        ZSerializer<String> serializer = new ZSerializer<>() {};
        var payload = serializer.serialize(stringInput);

        ZDeserializer<String> deserializer = new ZDeserializer<>() {};
        String stringOutput = deserializer.deserialize(payload);

        assertEquals(stringInput, stringOutput);
    }

    @Test
    public void testByteArraySerializationAndDeserialization() {
        byte[] byteArrayInput = "example".getBytes();

        ZSerializer<byte[]> serializer = new ZSerializer<>() {};
        var payload = serializer.serialize(byteArrayInput);

        ZDeserializer<byte[]> deserializer = new ZDeserializer<>() {};
        byte[] byteArrayOutput = deserializer.deserialize(payload);

        assertTrue(Arrays.equals(byteArrayInput, byteArrayOutput));
    }

    @Test
    public void testListOfStringsSerializationAndDeserialization() {
        List<String> inputList = List.of("sample1", "sample2", "sample3");

        ZSerializer<List<String>> serializer = new ZSerializer<>() {};
        var payload = serializer.serialize(inputList);

        ZDeserializer<List<String>> deserializer = new ZDeserializer<>() {};
        List<String> outputList = deserializer.deserialize(payload);

        assertEquals(inputList, outputList);
    }

    @Test
    public void testListOfByteArraysSerializationAndDeserialization() {
        List<byte[]> inputList = Stream.of("sample1", "sample2", "sample3")
                .map(String::getBytes)
                .collect(Collectors.toList());

        ZSerializer<List<byte[]>> serializer = new ZSerializer<>() {};
        var payload = serializer.serialize(inputList);

        ZDeserializer<List<byte[]>> deserializer = new ZDeserializer<>() {};
        List<byte[]> outputList = deserializer.deserialize(payload);

        assertTrue(compareByteArrayLists(inputList, outputList));
    }

    @Test
    public void testMapOfStringsSerializationAndDeserialization() {
        Map<String, String> inputMap = Map.of("key1", "value1", "key2", "value2", "key3", "value3");

        // Create serializer
        ZSerializer<Map<String, String>> serializer = new ZSerializer<>() {};
        var payload = serializer.serialize(inputMap);

        // Create deserializer
        ZDeserializer<Map<String, String>> deserializer = new ZDeserializer<>() {};
        Map<String, String> outputMap = deserializer.deserialize(payload);

        assertEquals(inputMap, outputMap);
    }

    /**********************************************
     * Additional test cases for new Kotlin types *
     **********************************************/

    @Test
    public void testBooleanSerializationAndDeserialization() {
        boolean booleanInput = true;

        // Create serializer
        ZSerializer<Boolean> serializer = new ZSerializer<>() {};
        var payload = serializer.serialize(booleanInput);

        // Create deserializer
        ZDeserializer<Boolean> deserializer = new ZDeserializer<>() {};
        boolean booleanOutput = deserializer.deserialize(payload);

        assertEquals(booleanInput, booleanOutput);
    }

    /**********************************************
     * Tests for collections with new types       *
     **********************************************/

    @Test
    public void testListOfBooleansSerializationAndDeserialization() {
        List<Boolean> listBooleanInput = List.of(true, false, true);

        // Create serializer
        ZSerializer<List<Boolean>> serializer = new ZSerializer<>() {};
        var payload = serializer.serialize(listBooleanInput);

        // Create deserializer
        ZDeserializer<List<Boolean>> deserializer = new ZDeserializer<>() {};
        List<Boolean> listBooleanOutput = deserializer.deserialize(payload);

        assertEquals(listBooleanInput, listBooleanOutput);
    }

    @Test
    public void testMapOfStringToListOfIntSerializationAndDeserialization() {
        Map<String, List<Integer>> mapOfListInput = Map.of("numbers", List.of(1, 2, 3, 4, 5));

        // Create serializer
        ZSerializer<Map<String, List<Integer>>> serializer = new ZSerializer<>() {};
        var payload = serializer.serialize(mapOfListInput);

        // Create deserializer
        ZDeserializer<Map<String, List<Integer>>> deserializer = new ZDeserializer<>() {};
        Map<String, List<Integer>> mapOfListOutput = deserializer.deserialize(payload);

        assertEquals(mapOfListInput, mapOfListOutput);
    }

    /*****************
     * Testing utils *
     *****************/

    private boolean compareByteArrayLists(List<byte[]> list1, List<byte[]> list2) {
        if (list1.size() != list2.size()) {
            return false;
        }
        for (int i = 0; i < list1.size(); i++) {
            if (!Arrays.equals(list1.get(i), list2.get(i))) {
                return false;
            }
        }
        return true;
    }
}
