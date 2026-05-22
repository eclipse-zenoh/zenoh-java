//
// Copyright (c) 2026 ZettaScale Technology
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

package io.zenoh

import io.zenoh.jni.JniZZBytes
import io.zenoh.jni.JniZZBytesKotlin
import kotlin.reflect.typeOf
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals

/**
 * Tests for Java/Kotlin serialization interoperability at the JNI bridge layer.
 *
 * Two groups:
 * 1. Kotlin round-trips — serialize and deserialize via [JniZZBytesKotlin] (KType path), covering
 *    all supported KotlinType variants including unsigned integers and Pair/Triple.
 * 2. Cross-path interop — verify that [JniZZBytes] (Java Type path) and [JniZZBytesKotlin]
 *    (KType path) produce identical wire bytes for the common types they share.
 */
class ZBytesInteropTests {

    // -------------------------------------------------------------------------
    // Group 1: Kotlin round-trips via JniZZBytesKotlin
    // -------------------------------------------------------------------------

    @Test
    fun testBooleanKotlinRoundTrip() {
        val input = true
        val bytes = JniZZBytesKotlin.serialize(input, typeOf<Boolean>())
        assertEquals(input, JniZZBytesKotlin.deserialize(bytes, typeOf<Boolean>()) as Boolean)
    }

    @Test
    fun testByteKotlinRoundTrip() {
        val input: Byte = 42
        val bytes = JniZZBytesKotlin.serialize(input, typeOf<Byte>())
        assertEquals(input, JniZZBytesKotlin.deserialize(bytes, typeOf<Byte>()) as Byte)
    }

    @Test
    fun testShortKotlinRoundTrip() {
        val input: Short = 1234
        val bytes = JniZZBytesKotlin.serialize(input, typeOf<Short>())
        assertEquals(input, JniZZBytesKotlin.deserialize(bytes, typeOf<Short>()) as Short)
    }

    @Test
    fun testIntKotlinRoundTrip() {
        val input = 1234567
        val bytes = JniZZBytesKotlin.serialize(input, typeOf<Int>())
        assertEquals(input, JniZZBytesKotlin.deserialize(bytes, typeOf<Int>()) as Int)
    }

    @Test
    fun testLongKotlinRoundTrip() {
        val input = 123456789012345L
        val bytes = JniZZBytesKotlin.serialize(input, typeOf<Long>())
        assertEquals(input, JniZZBytesKotlin.deserialize(bytes, typeOf<Long>()) as Long)
    }

    @Test
    fun testFloatKotlinRoundTrip() {
        val input = 3.1415f
        val bytes = JniZZBytesKotlin.serialize(input, typeOf<Float>())
        assertEquals(input, JniZZBytesKotlin.deserialize(bytes, typeOf<Float>()) as Float, 0.0001f)
    }

    @Test
    fun testDoubleKotlinRoundTrip() {
        val input = 2.718281828
        val bytes = JniZZBytesKotlin.serialize(input, typeOf<Double>())
        assertEquals(input, JniZZBytesKotlin.deserialize(bytes, typeOf<Double>()) as Double, 0.000000001)
    }

    @Test
    fun testStringKotlinRoundTrip() {
        val input = "hello zenoh"
        val bytes = JniZZBytesKotlin.serialize(input, typeOf<String>())
        assertEquals(input, JniZZBytesKotlin.deserialize(bytes, typeOf<String>()) as String)
    }

    @Test
    fun testByteArrayKotlinRoundTrip() {
        val input = byteArrayOf(1, 2, 3, 4, 5)
        val bytes = JniZZBytesKotlin.serialize(input, typeOf<ByteArray>())
        assertContentEquals(input, JniZZBytesKotlin.deserialize(bytes, typeOf<ByteArray>()) as ByteArray)
    }

    // Kotlin-only unsigned types

    @Test
    fun testUByteKotlinRoundTrip() {
        val input: UByte = 200u
        val bytes = JniZZBytesKotlin.serialize(input, typeOf<UByte>())
        assertEquals(input, JniZZBytesKotlin.deserialize(bytes, typeOf<UByte>()) as UByte)
    }

    @Test
    fun testUShortKotlinRoundTrip() {
        val input: UShort = 60000u
        val bytes = JniZZBytesKotlin.serialize(input, typeOf<UShort>())
        assertEquals(input, JniZZBytesKotlin.deserialize(bytes, typeOf<UShort>()) as UShort)
    }

    @Test
    fun testUIntKotlinRoundTrip() {
        val input: UInt = 3000000000u
        val bytes = JniZZBytesKotlin.serialize(input, typeOf<UInt>())
        assertEquals(input, JniZZBytesKotlin.deserialize(bytes, typeOf<UInt>()) as UInt)
    }

    @Test
    fun testULongKotlinRoundTrip() {
        val input: ULong = 10000000000000000000u
        val bytes = JniZZBytesKotlin.serialize(input, typeOf<ULong>())
        assertEquals(input, JniZZBytesKotlin.deserialize(bytes, typeOf<ULong>()) as ULong)
    }

    // Collections

    @Test
    fun testListOfIntKotlinRoundTrip() {
        val input = listOf(1, 2, 3, 4, 5)
        val bytes = JniZZBytesKotlin.serialize(input, typeOf<List<Int>>())
        @Suppress("UNCHECKED_CAST")
        assertEquals(input, JniZZBytesKotlin.deserialize(bytes, typeOf<List<Int>>()) as List<Int>)
    }

    @Test
    fun testListOfStringKotlinRoundTrip() {
        val input = listOf("a", "b", "c")
        val bytes = JniZZBytesKotlin.serialize(input, typeOf<List<String>>())
        @Suppress("UNCHECKED_CAST")
        assertEquals(input, JniZZBytesKotlin.deserialize(bytes, typeOf<List<String>>()) as List<String>)
    }

    @Test
    fun testMapOfStringToIntKotlinRoundTrip() {
        val input = mapOf("one" to 1, "two" to 2, "three" to 3)
        val bytes = JniZZBytesKotlin.serialize(input, typeOf<Map<String, Int>>())
        @Suppress("UNCHECKED_CAST")
        assertEquals(input, JniZZBytesKotlin.deserialize(bytes, typeOf<Map<String, Int>>()) as Map<String, Int>)
    }

    // Kotlin-only compound types

    @Test
    fun testPairKotlinRoundTrip() {
        val input = Pair(42, "hello")
        val bytes = JniZZBytesKotlin.serialize(input, typeOf<Pair<Int, String>>())
        @Suppress("UNCHECKED_CAST")
        assertEquals(input, JniZZBytesKotlin.deserialize(bytes, typeOf<Pair<Int, String>>()) as Pair<Int, String>)
    }

    @Test
    fun testTripleKotlinRoundTrip() {
        val input = Triple("zenoh", 99, true)
        val bytes = JniZZBytesKotlin.serialize(input, typeOf<Triple<String, Int, Boolean>>())
        @Suppress("UNCHECKED_CAST")
        assertEquals(input, JniZZBytesKotlin.deserialize(bytes, typeOf<Triple<String, Int, Boolean>>()) as Triple<String, Int, Boolean>)
    }

    // -------------------------------------------------------------------------
    // Group 2: Cross-path interop — Java Type path ↔ KType path
    //
    // For the Java path, java.lang.Class<T> implements java.lang.reflect.Type.
    // Boxed types give the qualified names zbytes.rs expects ("java.lang.Integer", etc.).
    // -------------------------------------------------------------------------

    @Test
    fun testBooleanJavaToKotlinInterop() {
        val input = true
        val bytes = JniZZBytes.serialize(input, java.lang.Boolean::class.java)
        assertEquals(input, JniZZBytesKotlin.deserialize(bytes, typeOf<Boolean>()) as Boolean)
    }

    @Test
    fun testBooleanKotlinToJavaInterop() {
        val input = true
        val bytes = JniZZBytesKotlin.serialize(input, typeOf<Boolean>())
        assertEquals(input, JniZZBytes.deserialize(bytes, java.lang.Boolean::class.java) as Boolean)
    }

    @Test
    fun testByteJavaToKotlinInterop() {
        val input: Byte = 42
        val bytes = JniZZBytes.serialize(input, java.lang.Byte::class.java)
        assertEquals(input, JniZZBytesKotlin.deserialize(bytes, typeOf<Byte>()) as Byte)
    }

    @Test
    fun testByteKotlinToJavaInterop() {
        val input: Byte = 42
        val bytes = JniZZBytesKotlin.serialize(input, typeOf<Byte>())
        assertEquals(input, JniZZBytes.deserialize(bytes, java.lang.Byte::class.java) as Byte)
    }

    @Test
    fun testShortJavaToKotlinInterop() {
        val input: Short = 1234
        val bytes = JniZZBytes.serialize(input, java.lang.Short::class.java)
        assertEquals(input, JniZZBytesKotlin.deserialize(bytes, typeOf<Short>()) as Short)
    }

    @Test
    fun testShortKotlinToJavaInterop() {
        val input: Short = 1234
        val bytes = JniZZBytesKotlin.serialize(input, typeOf<Short>())
        assertEquals(input, JniZZBytes.deserialize(bytes, java.lang.Short::class.java) as Short)
    }

    @Test
    fun testIntJavaToKotlinInterop() {
        val input = 42
        val bytes = JniZZBytes.serialize(input, java.lang.Integer::class.java)
        assertEquals(input, JniZZBytesKotlin.deserialize(bytes, typeOf<Int>()) as Int)
    }

    @Test
    fun testIntKotlinToJavaInterop() {
        val input = 42
        val bytes = JniZZBytesKotlin.serialize(input, typeOf<Int>())
        assertEquals(input, JniZZBytes.deserialize(bytes, java.lang.Integer::class.java) as Int)
    }

    @Test
    fun testLongJavaToKotlinInterop() {
        val input = 123456789012345L
        val bytes = JniZZBytes.serialize(input, java.lang.Long::class.java)
        assertEquals(input, JniZZBytesKotlin.deserialize(bytes, typeOf<Long>()) as Long)
    }

    @Test
    fun testLongKotlinToJavaInterop() {
        val input = 123456789012345L
        val bytes = JniZZBytesKotlin.serialize(input, typeOf<Long>())
        assertEquals(input, JniZZBytes.deserialize(bytes, java.lang.Long::class.java) as Long)
    }

    @Test
    fun testFloatJavaToKotlinInterop() {
        val input = 3.1415f
        val bytes = JniZZBytes.serialize(input, java.lang.Float::class.java)
        assertEquals(input, JniZZBytesKotlin.deserialize(bytes, typeOf<Float>()) as Float, 0.0001f)
    }

    @Test
    fun testFloatKotlinToJavaInterop() {
        val input = 3.1415f
        val bytes = JniZZBytesKotlin.serialize(input, typeOf<Float>())
        assertEquals(input, JniZZBytes.deserialize(bytes, java.lang.Float::class.java) as Float, 0.0001f)
    }

    @Test
    fun testDoubleJavaToKotlinInterop() {
        val input = 2.718281828
        val bytes = JniZZBytes.serialize(input, java.lang.Double::class.java)
        assertEquals(input, JniZZBytesKotlin.deserialize(bytes, typeOf<Double>()) as Double, 0.000000001)
    }

    @Test
    fun testDoubleKotlinToJavaInterop() {
        val input = 2.718281828
        val bytes = JniZZBytesKotlin.serialize(input, typeOf<Double>())
        assertEquals(input, JniZZBytes.deserialize(bytes, java.lang.Double::class.java) as Double, 0.000000001)
    }

    @Test
    fun testStringJavaToKotlinInterop() {
        val input = "hello zenoh"
        val bytes = JniZZBytes.serialize(input, java.lang.String::class.java)
        assertEquals(input, JniZZBytesKotlin.deserialize(bytes, typeOf<String>()) as String)
    }

    @Test
    fun testStringKotlinToJavaInterop() {
        val input = "hello zenoh"
        val bytes = JniZZBytesKotlin.serialize(input, typeOf<String>())
        assertEquals(input, JniZZBytes.deserialize(bytes, java.lang.String::class.java) as String)
    }

    @Test
    fun testByteArrayJavaToKotlinInterop() {
        val input = byteArrayOf(10, 20, 30, 40, 50)
        val bytes = JniZZBytes.serialize(input, ByteArray::class.java)
        assertContentEquals(input, JniZZBytesKotlin.deserialize(bytes, typeOf<ByteArray>()) as ByteArray)
    }

    @Test
    fun testByteArrayKotlinToJavaInterop() {
        val input = byteArrayOf(10, 20, 30, 40, 50)
        val bytes = JniZZBytesKotlin.serialize(input, typeOf<ByteArray>())
        assertContentEquals(input, JniZZBytes.deserialize(bytes, ByteArray::class.java) as ByteArray)
    }
}
