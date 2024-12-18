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
 * ```java
 * var queue = session.declareSubscriber("a/b/c");
 * try (Session session = Zenoh.open(config)) {
 *     try (var subscriber = session.declareSubscriber(keyExpr)) {
 *         var receiver = subscriber.getReceiver();
 *         assert receiver != null;
 *         while (true) {
 *             Optional<Sample> wrapper = receiver.take();
 *             if (wrapper.isEmpty()) {
 *                 break;
 *             }
 *             System.out.println(wrapper.get());
 *         }
 *     }
 * }
 * ```
 *
 * Example using a callback:
 * ```java
 * try (Session session = Zenoh.open(config)) {
 *     session.declareSubscriber(keyExpr, System.out::println);
 * }
 * ```
 *
 * Example using a handler:
 * ```java
 * class MyHandler implements Handler<Sample, ArrayList<Sample>> {...}
 *
 * //...
 * try (Session session = Zenoh.open(config)) {
 *     var handler = new MyHandler();
 *     var arraylist = session.declareSubscriber(keyExpr, handler);
 *     // ...
 * }
 * ```
 */
sealed class Subscriber(
    val keyExpr: KeyExpr, private var jniSubscriber: JNISubscriber?
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

/**
 * Subscriber using a callback to handle incoming samples.
 *
 * Example:
 * ```java
 * try (Session session = Zenoh.open(config)) {
 *     session.declareSubscriber(keyExpr, System.out::println);
 * }
 * ```
 */
class CallbackSubscriber internal constructor(keyExpr: KeyExpr, jniSubscriber: JNISubscriber?): Subscriber(keyExpr, jniSubscriber)

/**
 * Subscriber using a [io.zenoh.handlers.Handler] for handling incoming samples.
 *
 * Example using the default handler:
 * ```java
 * try (Session session = Zenoh.open(config)) {
 *     try (HandlerSubscriber<BlockingQueue<Optional<Sample>>> subscriber = session.declareSubscriber(keyExpr)) {
 *         BlockingQueue<Optional<Sample>> receiver = subscriber.getReceiver();
 *         assert receiver != null;
 *         while (true) {
 *             Optional<Sample> wrapper = receiver.take();
 *             if (wrapper.isEmpty()) {
 *                 break;
 *             }
 *             System.out.println(wrapper.get());
 *         }
 *     }
 * }
 * ```
 *
 * @param R The type of the receiver.
 * @param receiver The receiver of the subscriber's handler.
 */
class HandlerSubscriber<R> internal constructor(keyExpr: KeyExpr, jniSubscriber: JNISubscriber?, val receiver: R): Subscriber(keyExpr, jniSubscriber)
