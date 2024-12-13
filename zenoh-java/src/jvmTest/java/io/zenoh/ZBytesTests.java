package io.zenoh;

import com.google.common.reflect.TypeToken;
import io.zenoh.ext.ZDeserializerKt;
import io.zenoh.ext.ZSerializerKt;
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
        var payload = ZSerializerKt.zSerialize(intInput, new TypeToken<>() {});
        int intOutput = ZDeserializerKt.zDeserialize(payload, new TypeToken<>() {});
        assertEquals(intInput, intOutput);
    }

    @Test
    public void testFloatSerializationAndDeserialization() {
        float floatInput = 3.1415f;
        var payload = ZSerializerKt.zSerialize(floatInput, new TypeToken<>() {});
        float floatOutput = ZDeserializerKt.zDeserialize(payload, new TypeToken<>() {});
        assertEquals(floatInput, floatOutput, 0.0001);
    }

    @Test
    public void testStringSerializationAndDeserialization() {
        String stringInput = "example";
        var payload = ZSerializerKt.zSerialize(stringInput, new TypeToken<>() {});
        String stringOutput = ZDeserializerKt.zDeserialize(payload, new TypeToken<>() {});
        assertEquals(stringInput, stringOutput);
    }

    @Test
    public void testByteArraySerializationAndDeserialization() {
        byte[] byteArrayInput = "example".getBytes();
        var payload = ZSerializerKt.zSerialize(byteArrayInput, new TypeToken<>() {});
        byte[] byteArrayOutput = ZDeserializerKt.zDeserialize(payload, new TypeToken<>() {});
        assertTrue(Arrays.equals(byteArrayInput, byteArrayOutput));
    }

    @Test
    public void testListOfStringsSerializationAndDeserialization() {
        List<String> inputList = List.of("sample1", "sample2", "sample3");
        var payload = ZSerializerKt.zSerialize(inputList, new TypeToken<>() {});
        List<String> outputList = ZDeserializerKt.zDeserialize(payload, new TypeToken<>() {});
        assertEquals(inputList, outputList);
    }

    @Test
    public void testListOfByteArraysSerializationAndDeserialization() {
        List<byte[]> inputList = Stream.of("sample1", "sample2", "sample3")
                .map(String::getBytes)
                .collect(Collectors.toList());
        var payload = ZSerializerKt.zSerialize(inputList, new TypeToken<>() {});
        List<byte[]> outputList = ZDeserializerKt.zDeserialize(payload, new TypeToken<>() {});
        assertTrue(compareByteArrayLists(inputList, outputList));
    }

    @Test
    public void testMapOfStringsSerializationAndDeserialization() {
        Map<String, String> inputMap = Map.of("key1", "value1", "key2", "value2", "key3", "value3");
        var payload = ZSerializerKt.zSerialize(inputMap, new TypeToken<>() {});
        Map<String, String> outputMap = ZDeserializerKt.zDeserialize(payload, new TypeToken<>() {});
        assertEquals(inputMap, outputMap);
    }

    /**********************************************
     * Additional test cases for new Kotlin types *
     **********************************************/

    @Test
    public void testBooleanSerializationAndDeserialization() {
        boolean booleanInput = true;
        var payload = ZSerializerKt.zSerialize(booleanInput, new TypeToken<>() {});
        boolean booleanOutput = ZDeserializerKt.zDeserialize(payload, new TypeToken<>() {});
        assertEquals(booleanInput, booleanOutput);
    }

    /**********************************************
     * Tests for collections with new types       *
     **********************************************/

    @Test
    public void testListOfBooleansSerializationAndDeserialization() {
        List<Boolean> listBooleanInput = List.of(true, false, true);
        var payload = ZSerializerKt.zSerialize(listBooleanInput, new TypeToken<>() {});
        List<Boolean> listBooleanOutput = ZDeserializerKt.zDeserialize(payload, new TypeToken<>() {});
        assertEquals(listBooleanInput, listBooleanOutput);
    }

    @Test
    public void testMapOfStringToListOfIntSerializationAndDeserialization() {
        Map<String, List<Integer>> mapOfListInput = Map.of("numbers", List.of(1, 2, 3, 4, 5));
        var payload = ZSerializerKt.zSerialize(mapOfListInput, new TypeToken<>() {});
        Map<String, List<Integer>> mapOfListOutput = ZDeserializerKt.zDeserialize(payload, new TypeToken<>() {});
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
