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
import io.zenoh.Resolvable
import io.zenoh.config.WhatAmI
import io.zenoh.handlers.BlockingQueueHandler
import io.zenoh.handlers.Callback
import io.zenoh.handlers.Handler
import io.zenoh.jni.JNIScout
import java.util.*
import java.util.concurrent.BlockingQueue

class ScoutBuilder<R> internal constructor(
    private var callback: Callback<Hello>? = null,
    private var handler: Handler<Hello, R>? = null,
    private var config: Config? = null,
    private var whatAmI: Set<WhatAmI> = setOf(WhatAmI.Peer, WhatAmI.Router)
): Resolvable<Scout<R>> {

    /**
     * Specify a [Callback] to be run when receiving a [Hello] message. Overrides any previously specified callback or handler.
     */
    fun callback(callback: Callback<Hello>): ScoutBuilder<Unit> =
        ScoutBuilder(callback = callback, handler = null, config = config, whatAmI = whatAmI)

    fun whatAmI(whatAmI: Set<WhatAmI>): ScoutBuilder<R> {
        return ScoutBuilder(callback, handler, config, whatAmI)
    }

    /**
     * Specify a [Handler]. Overrides any previously specified callback or handler.
     */
    fun <R2> with(handler: Handler<Hello, R2>): ScoutBuilder<R2> =
        ScoutBuilder(callback = null, handler = handler, config = config, whatAmI = whatAmI)

    /** Specify a [BlockingQueue]. Overrides any previously specified callback or handler. */
    fun with(blockingQueue: BlockingQueue<Optional<Hello>>): ScoutBuilder<BlockingQueue<Optional<Hello>>> =
        ScoutBuilder(callback = null, handler = BlockingQueueHandler(blockingQueue), config = config, whatAmI = whatAmI)

    /**
     * Resolve the builder, creating a [Scout] with the provided parameters.
     *
     * @return The newly created [Scout].
     */
    override fun res(): Scout<R> {
        require(callback != null || handler != null) { "Either a callback or a handler must be provided." }
        val resolvedCallback = callback ?: Callback { t: Hello -> handler?.handle(t) }
        @Suppress("UNCHECKED_CAST")
        return JNIScout.scout(whatAmI = whatAmI, callback = resolvedCallback, onClose = fun() {
            handler?.onClose()
        }, receiver = handler?.receiver() ?: Unit as R, config = config)
    }
}
