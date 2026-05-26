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
import io.zenoh.exceptions.wrapJNIExceptionAsZError
import io.zenoh.handlers.Callback
import io.zenoh.config.ZenohId
import io.zenoh.scouting.CallbackScout
import io.zenoh.scouting.HandlerScout
import io.zenoh.scouting.Hello
import io.zenoh.config.WhatAmI
import io.zenoh.jni.callbacks.HelloCallback

/**
 * Adapter holding a generated [ZScout] handle, with companion helpers that
 * bridge the public scouting surface to the auto-generated zenoh-flat API.
 */
internal class JNIScout(internal val zScout: ZScout) {

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
            receiver: R,
        ): HandlerScout<R> = wrapJNIExceptionAsZError {
            HandlerScout(JNIScout(runScout(whatAmI, config, callback, onClose)), receiver)
        }

        @Throws(ZError::class)
        fun scoutWithCallback(
            whatAmI: Set<WhatAmI>,
            callback: Callback<Hello>,
            config: Config?,
        ): CallbackScout = wrapJNIExceptionAsZError {
            CallbackScout(JNIScout(runScout(whatAmI, config, callback) {}))
        }

        private fun runScout(
            whatAmI: Set<WhatAmI>,
            config: Config?,
            callback: Callback<Hello>,
            onClose: () -> Unit,
        ): ZScout {
            val bitfield = whatAmI.map { it.value }.reduce { acc, v -> acc or v }
            val helloCallback = HelloCallback { jniHello ->
                callback.run(
                    Hello(
                        whatAmI = WhatAmI.fromInt(jniHello.whatami.value),
                        zid = ZenohId(jniHello.zid.bytes),
                        locators = jniHello.locators,
                    )
                )
            }
            val onCloseCallback = io.zenoh.jni.callbacks.Callback { onClose() }
            return ZScout.scout(bitfield, config?.zConfig, helloCallback, onCloseCallback)
        }
    }

    fun close() {
        zScout.close()
    }
}
