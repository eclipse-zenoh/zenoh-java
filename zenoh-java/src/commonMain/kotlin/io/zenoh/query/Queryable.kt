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

package io.zenoh.query

import io.zenoh.*
import io.zenoh.exceptions.ZError
import io.zenoh.handlers.Callback
import io.zenoh.handlers.BlockingQueueHandler
import io.zenoh.handlers.Handler
import io.zenoh.jni.JNIQueryable
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.session.SessionDeclaration
import java.util.*
import java.util.concurrent.BlockingQueue
import java.util.concurrent.LinkedBlockingDeque

/**
 * A queryable that allows to perform multiple queries on the specified [KeyExpr].
 *
 * Its main purpose is to keep the queryable active as long as it exists.
 *
 * Example using the default [BlockingQueueHandler] handler:
 * ```java
 * try (Session session = Session.open()) {
 *     try (KeyExpr keyExpr = KeyExpr.tryFrom("demo/example/zenoh-java-queryable")) {
 *         System.out.println("Declaring Queryable");
 *         try (Queryable<BlockingQueue<Optional<Query>>> queryable = session.declareQueryable(keyExpr).res()) {
 *             BlockingQueue<Optional<Query>> receiver = queryable.getReceiver();
 *             while (true) {
 *                 Optional<Query> wrapper = receiver.take();
 *                 if (wrapper.isEmpty()) {
 *                     break;
 *                 }
 *                 Query query = wrapper.get();
 *                 String valueInfo = query.getValue() != null ? " with value '" + query.getValue() + "'" : "";
 *                 System.out.println(">> [Queryable] Received Query '" + query.getSelector() + "'" + valueInfo);
 *                 try {
 *                     query.reply(keyExpr)
 *                         .success("Queryable from Java!")
 *                         .withKind(SampleKind.PUT)
 *                         .withTimeStamp(TimeStamp.getCurrentTime())
 *                         .res();
 *                 } catch (Exception e) {
 *                     System.out.println(">> [Queryable] Error sending reply: " + e);
 *                 }
 *             }
 *         }
 *     }
 * }
 * ```
 *
 * @param R Receiver type of the [Handler] implementation. If no handler is provided to the builder, [R] will be [Unit].
 * @property keyExpr The [KeyExpr] to which the subscriber is associated.
 * @property receiver Optional [R] that is provided when specifying a [Handler] for the subscriber.
 * @property jniQueryable Delegate object in charge of communicating with the underlying native code.
 * @constructor Internal constructor. Instances of Queryable must be created through the [Builder] obtained after
 * calling [Session.declareQueryable] or alternatively through [newBuilder].
 */
class Queryable<R> internal constructor(
    val keyExpr: KeyExpr, val receiver: R?, private var jniQueryable: JNIQueryable?
) : AutoCloseable, SessionDeclaration {

    fun isValid(): Boolean {
        return jniQueryable != null
    }

    override fun undeclare() {
        jniQueryable?.close()
        jniQueryable = null
    }

    override fun close() {
        undeclare()
    }

    protected fun finalize() {
        jniQueryable?.close()
    }
}

/**
 * TODO: add doc
 */
data class QueryableConfig(
    var complete: Boolean = false,
    var onClose: Runnable? = null
) {
    fun complete(complete: Boolean) = apply { this.complete = complete }
    fun onClose(onClose: Runnable) = apply { this.onClose = onClose }
}
