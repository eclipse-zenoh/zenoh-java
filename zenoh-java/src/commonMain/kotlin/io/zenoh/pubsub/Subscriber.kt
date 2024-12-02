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

package io.zenoh.pubsub

import io.zenoh.handlers.BlockingQueueHandler
import io.zenoh.jni.JNISubscriber
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.session.SessionDeclaration

/**
 * A subscriber that allows listening to updates on a key expression and reacting to changes.
 *
 * Its main purpose is to keep the subscription active as long as it exists.
 *
 * Example using the default [BlockingQueueHandler] handler:
 *
 * TODO
 */
class Subscriber<R> internal constructor(
    val keyExpr: KeyExpr, val receiver: R?, private var jniSubscriber: JNISubscriber?
) : AutoCloseable, SessionDeclaration {

    fun isValid(): Boolean {
        return jniSubscriber != null
    }

    override fun undeclare() {
        jniSubscriber?.close()
        jniSubscriber = null
    }

    override fun close() {
        undeclare()
    }

    protected fun finalize() {
        jniSubscriber?.close()
    }
}
