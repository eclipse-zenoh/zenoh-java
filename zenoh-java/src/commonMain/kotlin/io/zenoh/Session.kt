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

package io.zenoh

import io.zenoh.bytes.IntoZBytes
import io.zenoh.config.ZenohId
import io.zenoh.exceptions.ZError
import io.zenoh.handlers.BlockingQueueHandler
import io.zenoh.handlers.Callback
import io.zenoh.handlers.Handler
import io.zenoh.jni.JNISession
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.liveliness.Liveliness
import io.zenoh.pubsub.*
import io.zenoh.query.*
import io.zenoh.query.Query
import io.zenoh.query.Queryable
import io.zenoh.sample.Sample
import io.zenoh.session.SessionDeclaration
import io.zenoh.session.SessionInfo
import java.util.*
import java.util.concurrent.BlockingQueue
import java.util.concurrent.LinkedBlockingDeque

/**
 * A Zenoh Session, the core interaction point with a Zenoh network.
 *
 * A session is typically associated with declarations such as [Publisher]s, [Subscriber]s, or [Queryable]s, which are
 * declared using [declarePublisher], [declareSubscriber], and [declareQueryable], respectively.
 * Other operations such as simple Put, Get or Delete can be performed from a session using [put], [get] and [delete].
 * Finally, it's possible to declare key expressions ([KeyExpr]) as well.
 *
 * Sessions are open upon creation and can be closed manually by calling [close]. Alternatively, the session will be
 * automatically closed when used with Java's try-with-resources statement or its Kotlin counterpart, [use].
 *
 * For optimal performance and adherence to good practices, it is recommended to have only one running session, which
 * is sufficient for most use cases. You should _never_ construct one session per publisher/subscriber, as this will
 * significantly increase the size of your Zenoh network, while preventing potential locality-based optimizations.
 */
class Session private constructor(private val config: Config) : AutoCloseable {

    internal var jniSession: JNISession? = JNISession()

    private var declarations = mutableListOf<SessionDeclaration>()

    companion object {

        internal val sessionClosedException = ZError("Session is closed.")

        /**
         * Open a [Session] with the provided [Config].
         *
         * @param config The configuration for the session.
         * @return The opened [Session].
         * @throws [ZError] in the case of a failure.
         */
        @Throws(ZError::class)
        internal fun open(config: Config): Session {
            val session = Session(config)
            return session.launch()
        }
    }

    /**
     * Close the session.
     *
     * Closing the session invalidates any attempt to perform a declaration or to perform an operation such as Put or Delete.
     * Attempting to do so will result in a failure.
     *
     * However, any session declaration that was still alive and bound to the session previous to closing it, will still be alive.
     */
    override fun close() {
        declarations.removeIf {
            it.undeclare()
            true
        }

        jniSession?.close()
        jniSession = null
    }

    @Suppress("removal")
    protected fun finalize() {
        close()
    }

    /**
     * Declare a [Publisher] on the session.
     *
     * TODO
     */
    @JvmOverloads
    @Throws(ZError::class)
    fun declarePublisher(keyExpr: KeyExpr, publisherOptions: PublisherOptions = PublisherOptions()): Publisher {
        return resolvePublisher(keyExpr, publisherOptions)
    }

    /**
     * Declare a [Subscriber] on the session.
     *
     * TODO
     */
    @Throws(ZError::class)
    fun declareSubscriber(keyExpr: KeyExpr): Subscriber<BlockingQueue<Optional<Sample>>> {
        return resolveSubscriberWithHandler(
            keyExpr,
            BlockingQueueHandler(LinkedBlockingDeque())
        )
    }

    /**
     * Declare a [Subscriber] on the session.
     *
     * TODO
     */
    @Throws(ZError::class)
    fun <R> declareSubscriber(keyExpr: KeyExpr, handler: Handler<Sample, R>): Subscriber<R> {
        return resolveSubscriberWithHandler(keyExpr, handler)
    }

    /**
     * Declare a [Subscriber] on the session.
     *
     * TODO
     */
    @Throws(ZError::class)
    fun declareSubscriber(keyExpr: KeyExpr, callback: Callback<Sample>): Subscriber<Void> {
        return resolveSubscriberWithCallback(keyExpr, callback)
    }

    /**
     * Declare a [Queryable] on the session.
     *
     * TODO
     */
    @Throws(ZError::class)
    @JvmOverloads
    fun declareQueryable(
        keyExpr: KeyExpr,
        options: QueryableOptions = QueryableOptions()
    ): Queryable<BlockingQueue<Optional<Query>>> {
        return resolveQueryableWithHandler(keyExpr, BlockingQueueHandler(LinkedBlockingDeque()), options)
    }

    /**
     * Declare a [Queryable] on the session.
     *
     * TODO
     */
    @Throws(ZError::class)
    @JvmOverloads
    fun <R> declareQueryable(keyExpr: KeyExpr, handler: Handler<Query, R>, options: QueryableOptions = QueryableOptions()): Queryable<R> {
        return resolveQueryableWithHandler(keyExpr, handler, options)
    }

    /**
     * Declare a [Queryable] on the session.
     *
     * TODO
     */
    @Throws(ZError::class)
    @JvmOverloads
    fun declareQueryable(keyExpr: KeyExpr, callback: Callback<Query>, options: QueryableOptions = QueryableOptions()): Queryable<Void> {
        return resolveQueryableWithCallback(keyExpr, callback, options)
    }

    /**
     * Declare a [KeyExpr].
     *
     * Informs Zenoh that you intend to use the provided Key Expression repeatedly.
     *
     * It is generally not needed to declare key expressions, as declaring a subscriber,
     * a queryable, or a publisher will also inform Zenoh of your intent to use their
     * key expressions repeatedly.
     *
     * Example:
     * ```java
     * try (Session session = session.open()) {
     *     try (KeyExpr keyExpr = session.declareKeyExpr("demo/java/example").res()) {
     *          Publisher publisher = session.declarePublisher(keyExpr).res();
     *          // ...
     *     }
     * }
     * ```
     *
     * @param keyExpr The intended Key expression.
     * @return A resolvable returning an optimized representation of the passed `keyExpr`.
     */
    fun declareKeyExpr(keyExpr: String): Resolvable<KeyExpr> = Resolvable { // TODO: remove resolvable
        return@Resolvable jniSession?.run {
            val keyexpr = declareKeyExpr(keyExpr)
            declarations.add(keyexpr)
            keyexpr
        } ?: throw sessionClosedException
    }

    /**
     * Undeclare a [KeyExpr].
     *
     * The key expression must have been previously declared on the session with [declareKeyExpr],
     * otherwise the operation will result in a failure.
     *
     * @param keyExpr The key expression to undeclare.
     * @return A resolvable returning the status of the undeclare operation.
     */
    fun undeclare(keyExpr: KeyExpr): Resolvable<Unit> = Resolvable {
        return@Resolvable jniSession?.run {
            undeclareKeyExpr(keyExpr)
        } ?: throw (sessionClosedException)
    }

    /**
     * Declare a [Get] with a [BlockingQueue] receiver.
     *
     * ```java
     * TODO: provide example
     * ```
     */
    @JvmOverloads
    @Throws(ZError::class)
    fun get(selector: IntoSelector, options: GetOptions = GetOptions()): BlockingQueue<Optional<Reply>> {
        val handler = BlockingQueueHandler<Reply>(LinkedBlockingDeque())
        return resolveGetWithHandler(
            selector,
            handler,
            options
        )
    }

    /**
     * TODO
     */
    @JvmOverloads
    @Throws(ZError::class)
    fun <R> get(selector: IntoSelector, handler: Handler<Reply, R>, options: GetOptions = GetOptions()): R {
        return resolveGetWithHandler(selector, handler, options)
    }

    /**
     * TODO
     */
    @JvmOverloads
    @Throws(ZError::class)
    fun get(selector: IntoSelector, callback: Callback<Reply>, options: GetOptions = GetOptions()) {
        return resolveGetWithCallback(selector, callback, options)
    }

    /**
     * Declare a [Put] with the provided value on the specified key expression.
     * //TODO update
     */
    @JvmOverloads
    @Throws(ZError::class)
    fun put(keyExpr: KeyExpr, payload: IntoZBytes, options: PutOptions = PutOptions()) {
        resolvePut(keyExpr, payload, options)
    }

    /**
     * TODO
     */
    @JvmOverloads
    @Throws(ZError::class)
    fun delete(keyExpr: KeyExpr, options: DeleteOptions = DeleteOptions()) {
        resolveDelete(keyExpr, options)
    }

    /** Returns if session is open or has been closed. */
    fun isClosed(): Boolean {
        return jniSession == null
    }

    /**
     * Returns the [SessionInfo] of this session.
     */
    fun info(): SessionInfo {
        return SessionInfo(this)
    }

    /**
     * Obtain a [Liveliness] instance tied to this Zenoh session.
     */
    fun liveliness(): Liveliness {
        return Liveliness(this)
    }

    @Throws(ZError::class)
    internal fun resolvePublisher(keyExpr: KeyExpr, options: PublisherOptions): Publisher {
        return jniSession?.run {
            val publisher = declarePublisher(keyExpr, options)
            declarations.add(publisher)
            publisher
        } ?: throw (sessionClosedException)
    }

    @Throws(ZError::class)
    internal fun <R> resolveSubscriberWithHandler(
        keyExpr: KeyExpr, handler: Handler<Sample, R>
    ): Subscriber<R> {
        return jniSession?.run {
            val subscriber = declareSubscriberWithHandler(keyExpr, handler)
            declarations.add(subscriber)
            subscriber
        } ?: throw (sessionClosedException)
    }

    @Throws(ZError::class)
    internal fun resolveSubscriberWithCallback(
        keyExpr: KeyExpr, callback: Callback<Sample>
    ): Subscriber<Void> {
        return jniSession?.run {
            val subscriber = declareSubscriberWithCallback(keyExpr, callback)
            declarations.add(subscriber)
            subscriber
        } ?: throw (sessionClosedException)
    }

    @Throws(ZError::class)
    internal fun <R> resolveQueryableWithHandler(
        keyExpr: KeyExpr, handler: Handler<Query, R>, options: QueryableOptions
    ): Queryable<R> {
        return jniSession?.run {
            val queryable = declareQueryableWithHandler(keyExpr, handler, options)
            declarations.add(queryable)
            queryable
        } ?: throw (sessionClosedException)
    }

    @Throws(ZError::class)
    internal fun resolveQueryableWithCallback(
        keyExpr: KeyExpr, callback: Callback<Query>, options: QueryableOptions
    ): Queryable<Void> {
        return jniSession?.run {
            val queryable = declareQueryableWithCallback(keyExpr, callback, options)
            declarations.add(queryable)
            queryable
        } ?: throw (sessionClosedException)
    }

    @Throws(ZError::class)
    internal fun <R> resolveGetWithHandler(
        selector: IntoSelector,
        handler: Handler<Reply, R>,
        options: GetOptions
    ): R {
        return jniSession?.performGetWithHandler(
            selector,
            handler,
            options
        ) ?: throw sessionClosedException
    }

    @Throws(ZError::class)
    internal fun resolveGetWithCallback(
        selector: IntoSelector,
        callback: Callback<Reply>,
        options: GetOptions
    ) {
        return jniSession?.performGetWithCallback(
            selector,
            callback,
            options
        ) ?: throw sessionClosedException
    }

    @Throws(ZError::class)
    internal fun resolvePut(keyExpr: KeyExpr, payload: IntoZBytes, putOptions: PutOptions) {
        jniSession?.run { performPut(keyExpr, payload, putOptions) }
    }

    @Throws(ZError::class)
    internal fun resolveDelete(keyExpr: KeyExpr, deleteOptions: DeleteOptions) {
        jniSession?.run { performDelete(keyExpr, deleteOptions) }
    }

    @Throws(ZError::class)
    internal fun zid(): ZenohId {
        return jniSession?.zid() ?: throw sessionClosedException
    }

    @Throws(ZError::class)
    internal fun getPeersId(): List<ZenohId> {
        return jniSession?.peersZid() ?: throw sessionClosedException
    }

    @Throws(ZError::class)
    internal fun getRoutersId(): List<ZenohId> {
        return jniSession?.routersZid() ?: throw sessionClosedException
    }

    /** Launches the session through the jni session, returning the [Session] on success. */
    @Throws(ZError::class)
    private fun launch(): Session {
        jniSession!!.open(config)
        return this
    }
}
