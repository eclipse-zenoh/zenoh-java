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

import io.zenoh.*
import io.zenoh.handlers.Callback
import io.zenoh.handlers.BlockingQueueHandler
import io.zenoh.handlers.Handler
import io.zenoh.jni.JNISubscriber
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.sample.Sample
import io.zenoh.session.SessionDeclaration
import java.util.*

/**
 * A subscriber that allows listening to updates on a key expression and reacting to changes.
 *
 * Its main purpose is to keep the subscription active as long as it exists.
 *
 * Example using the default [BlockingQueueHandler] handler:
 *
 * ```java
 * System.out.println("Opening session...");
 * try (Session session = Session.open()) {
 *     try (KeyExpr keyExpr = KeyExpr.tryFrom("demo/example")) {
 *         System.out.println("Declaring Subscriber on '" + keyExpr + "'...");
 *         try (Subscriber<BlockingQueue<Optional<Sample>>> subscriber = session.declareSubscriber(keyExpr).res()) {
 *             BlockingQueue<Optional<Sample>> receiver = subscriber.getReceiver();
 *             assert receiver != null;
 *             while (true) {
 *                 Optional<Sample> wrapper = receiver.take();
 *                 if (wrapper.isEmpty()) {
 *                     break;
 *                 }
 *                 Sample sample = wrapper.get();
 *                 System.out.println(">> [Subscriber] Received " + sample.getKind() + " ('" + sample.getKeyExpr() + "': '" + sample.getValue() + "')");
 *             }
 *         }
 *     }
 * }
 * ```
 *
 * @param R Receiver type of the [Handler] implementation. If no handler is provided to the builder, R will be [Unit].
 * @property keyExpr The [KeyExpr] to which the subscriber is associated.
 * @property receiver Optional [R] that is provided when specifying a [Handler] for the subscriber.
 * @property jniSubscriber Delegate object in charge of communicating with the underlying native code.
 * @constructor Internal constructor. Instances of Subscriber must be created through the [Builder] obtained after
 * calling [Session.declareSubscriber] or alternatively through [newBuilder].
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

data class SubscriberConfig<R>(
    var callback: Callback<Sample>? = null,
    var handler: Handler<Sample, R>? = null,
    var onClose: (() -> Unit)? = null
)
