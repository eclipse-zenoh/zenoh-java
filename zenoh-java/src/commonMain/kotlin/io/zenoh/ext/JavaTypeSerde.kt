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

package io.zenoh.ext

import io.zenoh.exceptions.ZError
import io.zenoh.jni.bytes.SerializationCodec
import java.lang.reflect.ParameterizedType
import java.lang.reflect.Type

/**
 * Build a [SerializationCodec.SerdeType] from a Guava `TypeToken`'s
 * [java.lang.reflect.Type] — the reflection adapter for zenoh-java's serializer.
 * Handles `Class<?>` and `ParameterizedType` for `List`/`Map`; the Kotlin-only
 * unsigned value classes and `Pair`/`Triple` erase in the Java reflection
 * representation and are not expressible here. Throws [ZError] on an
 * unsupported type (argument preparation — before the byte codec runs).
 */
internal fun serdeTypeOfJava(type: Type): SerializationCodec.SerdeType = when (type) {
    is Class<*> -> when (type.name) {
        "java.lang.Boolean", "boolean" -> SerializationCodec.SerdeType.Bool
        "java.lang.Byte", "byte" -> SerializationCodec.SerdeType.I8
        "java.lang.Short", "short" -> SerializationCodec.SerdeType.I16
        "java.lang.Integer", "int" -> SerializationCodec.SerdeType.I32
        "java.lang.Long", "long" -> SerializationCodec.SerdeType.I64
        "java.lang.Float", "float" -> SerializationCodec.SerdeType.F32
        "java.lang.Double", "double" -> SerializationCodec.SerdeType.F64
        "java.lang.String" -> SerializationCodec.SerdeType.Str
        "[B" -> SerializationCodec.SerdeType.Bytes
        else -> throw ZError("Unsupported type: ${type.name}")
    }
    is ParameterizedType -> {
        val raw = type.rawType as? Class<*> ?: throw ZError("Unsupported raw type: ${type.rawType}")
        val args = type.actualTypeArguments
        when {
            List::class.java.isAssignableFrom(raw) && args.size == 1 ->
                SerializationCodec.SerdeType.ZList(serdeTypeOfJava(args[0]))
            Map::class.java.isAssignableFrom(raw) && args.size == 2 ->
                SerializationCodec.SerdeType.ZMap(serdeTypeOfJava(args[0]), serdeTypeOfJava(args[1]))
            else -> throw ZError("Unsupported parameterized type: ${raw.name}")
        }
    }
    else -> throw ZError("Unsupported type: $type")
}
