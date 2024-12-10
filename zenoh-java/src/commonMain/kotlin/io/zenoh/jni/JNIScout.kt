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

package io.zenoh.jni

import io.zenoh.Config
import io.zenoh.ZenohLoad
import io.zenoh.exceptions.ZError
import io.zenoh.handlers.Callback
import io.zenoh.jni.callbacks.JNIScoutCallback
import io.zenoh.config.ZenohId
import io.zenoh.scouting.Hello
import io.zenoh.config.WhatAmI
import io.zenoh.jni.callbacks.JNIOnCloseCallback
import io.zenoh.scouting.CallbackScout
import io.zenoh.scouting.HandlerScout

/**
 * Adapter class to handle the interactions with Zenoh through JNI for a [io.zenoh.scouting.Scout]
 *
 * @property ptr: raw pointer to the underlying native scout.
 */
internal class JNIScout(private val ptr: Long) {

    companion object {

        init {
            ZenohLoad
        }

        @Throws(ZError::class)
        fun <R> scoutWithHandler(
            whatAmI: Set<WhatAmI>,
            callback: Callback<Hello>,
            onClose: () -> Unit,
            config: Config?,
            receiver: R
        ): HandlerScout<R> {
            val scoutCallback = JNIScoutCallback { whatAmI2: Int, id: ByteArray, locators: List<String> ->
                callback.run(Hello(WhatAmI.fromInt(whatAmI2), ZenohId(id), locators))
            }
            val binaryWhatAmI: Int = whatAmI.map { it.value }.reduce { acc, it -> acc or it }
            val ptr = scoutViaJNI(binaryWhatAmI, scoutCallback, onClose,config?.jniConfig?.ptr ?: 0)
            return HandlerScout(JNIScout(ptr), receiver)
        }

        @Throws(ZError::class)
        fun scoutWithCallback(
            whatAmI: Set<WhatAmI>,
            callback: Callback<Hello>,
            config: Config?,
        ): CallbackScout {
            val scoutCallback = JNIScoutCallback { whatAmI2: Int, id: ByteArray, locators: List<String> ->
                callback.run(Hello(WhatAmI.fromInt(whatAmI2), ZenohId(id), locators))
            }
            val binaryWhatAmI: Int = whatAmI.map { it.value }.reduce { acc, it -> acc or it }
            val ptr = scoutViaJNI(binaryWhatAmI, scoutCallback, fun() {},config?.jniConfig?.ptr ?: 0)
            return CallbackScout(JNIScout(ptr))
        }

        @Throws(ZError::class)
        private external fun scoutViaJNI(
            whatAmI: Int,
            callback: JNIScoutCallback,
            onClose: JNIOnCloseCallback,
            configPtr: Long,
        ): Long

        @Throws(ZError::class)
        external fun freePtrViaJNI(ptr: Long)
    }

    fun close() {
        freePtrViaJNI(ptr)
    }
}
