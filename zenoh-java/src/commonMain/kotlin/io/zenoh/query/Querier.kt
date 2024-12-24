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

import io.zenoh.annotations.Unstable
import io.zenoh.bytes.Encoding
import io.zenoh.bytes.IntoZBytes
import io.zenoh.exceptions.ZError
import io.zenoh.handlers.BlockingQueueHandler
import io.zenoh.handlers.Callback
import io.zenoh.handlers.Handler
import io.zenoh.jni.JNIQuerier
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.qos.CongestionControl
import io.zenoh.qos.Priority
import io.zenoh.qos.QoS
import io.zenoh.session.SessionDeclaration
import java.time.Duration
import java.util.Optional
import java.util.concurrent.BlockingQueue
import java.util.concurrent.LinkedBlockingDeque

/**
 * A querier that allows to send queries to a [Queryable].
 *
 * Queriers are automatically undeclared when dropped.
 *
 * Example:
 * ```java
 * try (Session session = Zenoh.open(config)) {
 *     QuerierOptions options = new QuerierOptions();
 *     options.setTarget(QueryTarget.BEST_MATCHING);
 *     Querier querier = session.declareQuerier(selector.getKeyExpr(), options);
 *     //...
 *     Querier.GetOptions options = new Querier.GetOptions();
 *     options.setPayload(ZBytes.from("Example payload"));
 *     querier.get(reply -> {...}, options);
 * }
 * ```
 *
 * @param keyExpr The [KeyExpr] of the querier.
 * @param qos The [QoS] configuration of the querier.
 */
@Unstable
class Querier internal constructor(val keyExpr: KeyExpr, val qos: QoS, private var jniQuerier: JNIQuerier?) :
    SessionDeclaration, AutoCloseable {

    /**
     * Get options for the [Querier].
     */
    data class GetOptions(
        var parameters: Parameters? = null,
        var payload: IntoZBytes? = null,
        var encoding: Encoding? = null,
        var attachment: IntoZBytes? = null
    )

    /**
     * Perform a get operation to the [keyExpr] from the Querier and pipe them into a blocking queue.
     */
    @Throws(ZError::class)
    fun get(
        options: GetOptions
    ): BlockingQueue<Optional<Reply>> {
        val handler = BlockingQueueHandler<Reply>(LinkedBlockingDeque())
        return resolveGetWithHandler(keyExpr, handler, options)
    }

    /**
     * Perform a get operation to the [keyExpr] from the Querier and handle the incoming replies
     * with the [callback] provided.
     *
     * @param callback [Callback] to handle the incoming replies.
     * @param options [GetOptions] to configure the get operation.
     */
    @Throws(ZError::class)
    fun get(
        callback: Callback<Reply>,
        options: GetOptions
    ) {
        resolveGetWithCallback(keyExpr, callback, options)
    }

    /**
     * Perform a get operation to the [keyExpr] from the Querier and handle the incoming replies
     * with the [handler] provided.
     *
     * @param handler [Handler] to handle the receiving replies to the query.
     * @param options [GetOptions] to configure the get operation.
     */
    @Throws(ZError::class)
    fun <R> get(
        handler: Handler<Reply, R>,
        options: GetOptions
    ): R {
        return resolveGetWithHandler(keyExpr, handler, options)
    }

    /**
     * Get the [QoS.congestionControl] of the querier.
     */
    fun congestionControl() = qos.congestionControl

    /**
     * Get the [QoS.priority] of the querier.
     */
    fun priority() = qos.priority

    /**
     * Undeclares the querier. After calling this function, the querier won't be valid anymore and get operations
     * performed on it will fail.
     */
    override fun undeclare() {
        jniQuerier?.close()
        jniQuerier = null
    }

    /**
     * Closes the querier. Equivalent to [undeclare], this function is automatically called when using
     * try-with-resources.
     */
    override fun close() {
        undeclare()
    }

    protected fun finalize() {
        undeclare()
    }

    private fun resolveGetWithCallback(keyExpr: KeyExpr, callback: Callback<Reply>, options: GetOptions) {
        jniQuerier?.performGetWithCallback(keyExpr, callback, options) ?: throw ZError("Querier is not valid.")
    }

    private fun <R> resolveGetWithHandler(keyExpr: KeyExpr, handler: Handler<Reply, R>, options: GetOptions): R {
        return jniQuerier?.performGetWithHandler(keyExpr, handler, options) ?: throw ZError("Querier is not valid.")
    }
}

/**
 * Options for the [Querier] configuration.
 */
data class QuerierOptions(
    var target: QueryTarget = QueryTarget.BEST_MATCHING,
    var consolidationMode: ConsolidationMode = ConsolidationMode.AUTO,
    var timeout: Duration = Duration.ofMillis(10000),
    var express: Boolean = QoS.defaultQoS.express,
    var congestionControl: CongestionControl = QoS.defaultQoS.congestionControl,
    var priority: Priority = QoS.defaultQoS.priority
)
