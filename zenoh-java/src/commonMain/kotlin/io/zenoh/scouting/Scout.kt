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

package io.zenoh.scouting

import io.zenoh.Config
import io.zenoh.ZenohLoad
import io.zenoh.config.WhatAmI
import io.zenoh.config.ZenohId
import io.zenoh.exceptions.ZError
import io.zenoh.exceptions.wrapJNIExceptionAsZError
import io.zenoh.handlers.Callback
import io.zenoh.jni.ZScout
import io.zenoh.jni.callbacks.HelloCallback

/**
 * Scout for routers and/or peers.
 *
 * Scout spawns a task that periodically sends scout messages and waits for Hello replies.
 * Drop the returned Scout to stop the scouting task.
 *
 * To launch a scout, use [io.zenoh.Zenoh.scout]:
 *
 * Example using the default blocking queue handler:
 * ```java
 *
 * var scoutOptions = new ScoutOptions();
 * scoutOptions.setWhatAmI(Set.of(WhatAmI.Peer, WhatAmI.Router));
 *
 * var scout = Zenoh.scout(scoutOptions);
 * BlockingQueue<Optional<Hello>> receiver = scout.getReceiver();
 *
 * try {
 *     while (true) {
 *         Optional<Hello> wrapper = receiver.take();
 *         if (wrapper.isEmpty()) {
 *             break;
 *         }
 *
 *         Hello hello = wrapper.get();
 *         System.out.println(hello);
 *     }
 * } finally {
 *     scout.stop();
 * }
 * ```
 *
 * Example using a callback:
 * ```java
 * var scoutOptions = new ScoutOptions();
 * scoutOptions.setWhatAmI(Set.of(WhatAmI.Peer, WhatAmI.Router));
 * Zenoh.scout(hello -> {
 *     //...
 *     System.out.println(hello);
 * }, scoutOptions);
 * ```
 *
 * @see CallbackScout
 * @see HandlerScout
 */
sealed class Scout (
    private var zScout: ZScout?
) : AutoCloseable {

    companion object {

        init {
            ZenohLoad
        }

        @Throws(ZError::class)
        internal fun <R> scoutWithHandler(
            whatAmI: Set<WhatAmI>,
            callback: Callback<Hello>,
            onClose: () -> Unit,
            config: Config?,
            receiver: R,
        ): HandlerScout<R> = wrapJNIExceptionAsZError {
            HandlerScout(runScout(whatAmI, config, callback, onClose), receiver)
        }

        @Throws(ZError::class)
        internal fun scoutWithCallback(
            whatAmI: Set<WhatAmI>,
            callback: Callback<Hello>,
            config: Config?,
        ): CallbackScout = wrapJNIExceptionAsZError {
            CallbackScout(runScout(whatAmI, config, callback) {})
        }

        private fun runScout(
            whatAmI: Set<WhatAmI>,
            config: Config?,
            callback: Callback<Hello>,
            onClose: () -> Unit,
        ): ZScout {
            val bitfield = whatAmI.map { it.jni.value }.reduce { acc, v -> acc or v }
            val helloCallback = HelloCallback { jniHello ->
                callback.run(
                    Hello(
                        whatAmI = WhatAmI.fromJni(jniHello.whatami),
                        zid = ZenohId(jniHello.zid),
                        locators = jniHello.locators,
                    )
                )
            }
            val onCloseCallback = io.zenoh.jni.callbacks.Callback { onClose() }
            return ZScout.scout(bitfield, config?.zConfig, helloCallback, onCloseCallback)
        }
    }

    /**
     * Stops the scouting.
     */
    fun stop() {
        zScout?.close()
        zScout = null
    }

    /**
     * Equivalent to [stop].
     */
    override fun close() {
        stop()
    }

    protected fun finalize() {
        stop()
    }
}

/**
 * Scout using a callback to handle incoming [Hello] messages.
 *
 * Example:
 * ```java
 * CallbackScout scout = Zenoh.scout(hello -> {...});
 * ```
 */
class CallbackScout internal constructor(zScout: ZScout) : Scout(zScout)

/**
 * Scout using a handler to handle incoming [Hello] messages.
 *
 * Example
 * ```java
 * HandlerScout<BlockingQueue<Optional<Hello>>> scout = Zenoh.scout();
 * ```
 *
 * @param R The type of the receiver.
 * @param receiver The receiver of the scout's handler.
 */
class HandlerScout<R> internal constructor(zScout: ZScout, val receiver: R) : Scout(zScout)
