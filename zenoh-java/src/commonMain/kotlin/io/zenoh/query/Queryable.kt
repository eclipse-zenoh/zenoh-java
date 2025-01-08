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

import io.zenoh.handlers.BlockingQueueHandler
import io.zenoh.handlers.Handler
import io.zenoh.jni.JNIQueryable
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.session.SessionDeclaration

/**
 * A queryable that allows to perform multiple queries on the specified [KeyExpr].
 *
 * Its main purpose is to keep the queryable active as long as it exists.
 *
 * Example using the default [BlockingQueueHandler] handler:
 * ```java
 * try (Session session = Zenoh.open(config)) {
 *     var queryable = session.declareQueryable(keyExpr);
 *     BlockingQueue<Optional<Query>> receiver = queryable.getReceiver();
 *     assert receiver != null;
 *     while (true) {
 *         Optional<Query> wrapper = receiver.take();
 *         if (wrapper.isEmpty()) {
 *             break;
 *         }
 *         Query query = wrapper.get();
 *         query.reply(query.getKeyExpr(), "Example reply");
 *     }
 * }
 * ```
 *
 * Example using a [io.zenoh.handlers.Callback]:
 * ```java
 * try (Session session = Zenoh.open(config)) {
 *     var queryable = session.declareQueryable(keyExpr, query -> query.reply(query.getKeyExpr(), "Example reply");
 * }
 * ```
 *
 * @property keyExpr The [KeyExpr] to which the subscriber is associated.
 * @property jniQueryable Delegate object in charge of communicating with the underlying native code.
 * @see CallbackQueryable
 * @see HandlerQueryable
 */
sealed class Queryable(
    val keyExpr: KeyExpr, private var jniQueryable: JNIQueryable?
) : AutoCloseable, SessionDeclaration {

    fun isValid(): Boolean {
        return jniQueryable != null
    }

    /**
     * Undeclares the queryable.
     */
    override fun undeclare() {
        jniQueryable?.close()
        jniQueryable = null
    }

    /**
     * Closes the queryable, equivalent to [undeclare]. This function is automatically called
     * when using try with resources.
     */
    override fun close() {
        undeclare()
    }

    protected fun finalize() {
        jniQueryable?.close()
    }
}

/**
 * [Queryable] receiving replies through a callback.
 *
 * Example
 * ```java
 * try (Session session = Zenoh.open(config)) {
 *     CallbackQueryable queryable = session.declareQueryable(keyExpr, query -> query.reply(query.getKeyExpr(), "Example reply");
 * }
 * ```
 */
class CallbackQueryable internal constructor(keyExpr: KeyExpr, jniQueryable: JNIQueryable?): Queryable(keyExpr, jniQueryable)

/**
 * [Queryable] receiving replies through a [Handler].
 *
 * Example using the default receiver:
 * ```java
 * try (Session session = Zenoh.open(config)) {
 *     Queryable<BlockingQueue<Optional<Query>>> queryable = session.declareQueryable(keyExpr);
 *     BlockingQueue<Optional<Query>> receiver = queryable.getReceiver();
 *     while (true) {
 *         Optional<Query> wrapper = receiver.take();
 *         if (wrapper.isEmpty()) {
 *             break;
 *         }
 *         Query query = wrapper.get();
 *         query.reply(query.getKeyExpr(), "Example reply");
 *     }
 * }
 * ```
 *
 * @param R The type of the handler's receiver.
 * @param receiver The receiver of the queryable's handler.
 */
class HandlerQueryable<R> internal constructor(keyExpr: KeyExpr, jniQueryable: JNIQueryable?, val receiver: R): Queryable(keyExpr, jniQueryable)

/**
 * Options for configuring a [Queryable].
 *
 * @param complete The completeness of the information the queryable provides.
 */
data class QueryableOptions(var complete: Boolean = false)
