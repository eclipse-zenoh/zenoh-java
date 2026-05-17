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

package io.zenoh.jni

import io.zenoh.ZenohLoad
import io.zenoh.exceptions.ZError

/** Adapter for the native Zenoh config. */
public class JNIConfig(initialPtr: Long) {
    private val handle = NativeHandle(initialPtr)

    /**
     * Pointer accessor for callers that need to pass the raw pointer
     * to another JNI method (e.g. `openSessionViaJNI(config.ptr)`).
     * Throws if the config has already been closed.
     */
    internal val ptr: Long
        get() = handle.withPtr { it }

    companion object {

        init {
            ZenohLoad
        }

        @Throws(ZError::class)
        fun loadDefault(): JNIConfig = JNIConfig(loadDefaultConfigViaJNI())

        @Throws(ZError::class)
        fun loadFromFile(path: String): JNIConfig = JNIConfig(loadConfigFileViaJNI(path))

        @Throws(ZError::class)
        fun loadFromJson(rawConfig: String): JNIConfig = JNIConfig(loadJsonConfigViaJNI(rawConfig))

        @Throws(ZError::class)
        fun loadFromYaml(rawConfig: String): JNIConfig = JNIConfig(loadYamlConfigViaJNI(rawConfig))

        @Throws(ZError::class)
        private external fun loadDefaultConfigViaJNI(): Long

        @Throws(ZError::class)
        private external fun loadConfigFileViaJNI(path: String): Long

        @Throws(ZError::class)
        private external fun loadJsonConfigViaJNI(rawConfig: String): Long

        @Throws(ZError::class)
        private external fun loadYamlConfigViaJNI(rawConfig: String): Long

        @Throws(ZError::class)
        private external fun getIdViaJNI(ptr: Long): ByteArray

        @Throws(ZError::class)
        private external fun insertJson5ViaJNI(ptr: Long, key: String, value: String): Long

        private external fun freePtrViaJNI(ptr: Long)

        @Throws(ZError::class)
        private external fun getJsonViaJNI(ptr: Long, key: String): String
    }

    fun close() = handle.close { freePtrViaJNI(it) }

    @Throws(ZError::class)
    fun getId(): ByteArray = handle.withPtr { ptr -> getIdViaJNI(ptr) }

    @Throws(ZError::class)
    fun getJson(key: String): String = handle.withPtr { ptr -> getJsonViaJNI(ptr, key) }

    @Throws(ZError::class)
    fun insertJson5(key: String, value: String) = handle.withPtr { ptr ->
        insertJson5ViaJNI(ptr, key, value)
    }
}
