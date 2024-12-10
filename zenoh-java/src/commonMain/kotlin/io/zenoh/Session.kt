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

    protected fun finalize() {
        close()
    }

    /**
     * Declare a [Publisher] on the session.
     *
     * Example:
     * ```java
     * try (Session session = Zenoh.open(config)) {
     *     // A publisher config can optionally be provided.
     *     PublisherOptions publisherOptions = new PublisherOptions();
     *     publisherOptions.setEncoding(Encoding.ZENOH_STRING);
     *     publisherOptions.setCongestionControl(CongestionControl.BLOCK);
     *     publisherOptions.setReliability(Reliability.RELIABLE);
     *
     *     // Declare the publisher
     *     Publisher publisher = session.declarePublisher(keyExpr, publisherOptions);
     *
     *     int idx = 0;
     *     while (true) {
     *         Thread.sleep(1000);
     *         String payload = String.format("[%4d] %s", idx, value);
     *         System.out.println("Putting Data ('" + keyExpr + "': '" + payload + "')...");
     *         publisher.put(ZBytes.from(payload));
     *         idx++;
     *     }
     * }
     * ```
     *
     * @param keyExpr The [KeyExpr] the publisher will be associated to.
     * @param publisherOptions Optional [PublisherOptions] to configure the publisher.
     * @return The declared [Publisher].
     */
    @JvmOverloads
    @Throws(ZError::class)
    fun declarePublisher(keyExpr: KeyExpr, publisherOptions: PublisherOptions = PublisherOptions()): Publisher {
        return resolvePublisher(keyExpr, publisherOptions)
    }

    /**
     * Declare a [Subscriber] on the session.
     *
     * Example with blocking queue (default receiver):
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
     *             handleSample(wrapper.get());
     *         }
     *     }
     * }
     * ```
     *
     * @param keyExpr The [KeyExpr] the subscriber will be associated to.
     * @return [HandlerSubscriber] with a [BlockingQueue] as a receiver.
     */
    @Throws(ZError::class)
    fun declareSubscriber(keyExpr: KeyExpr): HandlerSubscriber<BlockingQueue<Optional<Sample>>> {
        return resolveSubscriberWithHandler(
            keyExpr,
            BlockingQueueHandler(LinkedBlockingDeque())
        )
    }

    /**
     * Declare a [Subscriber] on the session using a handler.
     *
     * Example with a custom handler:
     * ```java
     * // Example handler that stores the received samples into a queue.
     * class QueueHandler implements Handler<Sample, ArrayDeque<Sample>> {
     *
     *     final ArrayDeque<Sample> queue = new ArrayDeque<>();
     *
     *     @Override
     *     public void handle(Sample t) {
     *         queue.add(t);
     *     }
     *
     *     @Override
     *     public ArrayDeque<Sample> receiver() {
     *         return queue;
     *     }
     *
     *     @Override
     *     public void onClose() {}
     * }
     *
     * // ...
     *
     * try (Session session = Zenoh.open(config)) {
     *     QueueHandler queueHandler = new QueueHandler();
     *     var subscriber = session.declareSubscriber(keyExpr, queueHandler);
     *     // ...
     * }
     * ```
     *
     * @param R the [handler]'s receiver type.
     * @param keyExpr The [KeyExpr] the subscriber will be associated to.
     * @param handler The [Handler] to process the incoming [Sample]s received by the subscriber.
     * @return A [HandlerSubscriber] with the [handler]'s receiver.
     */
    @Throws(ZError::class)
    fun <R> declareSubscriber(keyExpr: KeyExpr, handler: Handler<Sample, R>): HandlerSubscriber<R> {
        return resolveSubscriberWithHandler(keyExpr, handler)
    }

    /**
     * Declare a [Subscriber] on the session using a callback.
     *
     * Example with a callback:
     * ```java
     * try (Session session = Zenoh.open(config)) {
     *     var subscriber = session.declareSubscriber(keyExpr, sample -> System.out.println(sample));
     *     // ...
     * }
     * ```
     *
     * @param keyExpr The [KeyExpr] the subscriber will be associated to.
     * @param callback [Callback] for handling the incoming samples.
     * @return A [CallbackSubscriber].
     */
    @Throws(ZError::class)
    fun declareSubscriber(keyExpr: KeyExpr, callback: Callback<Sample>): CallbackSubscriber {
        return resolveSubscriberWithCallback(keyExpr, callback)
    }

    /**
     * Declare a [Queryable] on the session.
     *
     * Example using a blocking queue (default receiver):
     * ```java
     * try (Session session = Zenoh.open(config)) {
     *     var queryable = session.declareQueryable(keyExpr);
     *     var receiver = queryable.getReceiver();
     *     while (true) {
     *         Optional<Query> wrapper = receiver.take();
     *         if (wrapper.isEmpty()) {
     *             break;
     *         }
     *         Query query = wrapper.get();
     *         query.reply(query.getKeyExpr(), ZBytes.from("Example reply));
     *     }
     * }
     * ```
     *
     * @param keyExpr The [KeyExpr] the queryable will be associated to.
     * @param options Optional [QueryableOptions] for configuring the queryable.
     * @return A [HandlerQueryable] with a [BlockingQueue] receiver.
     */
    @Throws(ZError::class)
    @JvmOverloads
    fun declareQueryable(
        keyExpr: KeyExpr,
        options: QueryableOptions = QueryableOptions()
    ): HandlerQueryable<BlockingQueue<Optional<Query>>> {
        return resolveQueryableWithHandler(keyExpr, BlockingQueueHandler(LinkedBlockingDeque()), options)
    }

    /**
     * Declare a [Queryable] on the session.
     *
     * Example using a custom [Handler]:
     * ```java
     * // Example handler that replies with the amount of queries received.
     * class QueryHandler implements Handler<Query, Void> {
     *
     *     private Int counter = 0;
     *
     *     @Override
     *     public void handle(Query query) {
     *          var keyExpr = query.getKeyExpr();
     *          query.reply(keyExpr, ZBytes.from("Reply #" + counter + "!"));
     *          counter++;
     *     }
     *
     *     @Override
     *     public Void receiver() {}
     *
     *     @Override
     *     public void onClose() {}
     * }
     *
     * // ...
     * try (Session session = Zenoh.open(config)) {
     *     var queryable = session.declareQueryable(keyExpr, new QueryHandler());
     *     //...
     * }
     * ```
     *
     * @param R The type of the [handler]'s receiver.
     * @param keyExpr The [KeyExpr] the queryable will be associated to.
     * @param handler The [Handler] to handle the incoming queries.
     * @param options Optional [QueryableOptions] for configuring the queryable.
     * @return A [HandlerQueryable] with the handler's receiver.
     */
    @Throws(ZError::class)
    @JvmOverloads
    fun <R> declareQueryable(keyExpr: KeyExpr, handler: Handler<Query, R>, options: QueryableOptions = QueryableOptions()): HandlerQueryable<R> {
        return resolveQueryableWithHandler(keyExpr, handler, options)
    }

    /**
     * Declare a [Queryable] on the session.
     *
     * ```java
     * try (Session session = Zenoh.open(config)) {
     *     var queryable = session.declareQueryable(keyExpr, query -> query.reply(keyExpr, ZBytes.from("Example reply")));
     *     //...
     * }
     * ```
     *
     * @param keyExpr The [KeyExpr] the queryable will be associated to.
     * @param callback The [Callback] to handle the incoming queries.
     * @param options Optional [QueryableOptions] for configuring the queryable.
     * @return A [CallbackQueryable].
     */
    @Throws(ZError::class)
    @JvmOverloads
    fun declareQueryable(keyExpr: KeyExpr, callback: Callback<Query>, options: QueryableOptions = QueryableOptions()): CallbackQueryable {
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
     * @param keyExpr The intended Key expression.
     * @return The declared [KeyExpr].
     */
    @Throws(ZError::class)
    fun declareKeyExpr(keyExpr: String): KeyExpr {
        return jniSession?.run {
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
    @Throws(ZError::class)
    fun undeclare(keyExpr: KeyExpr) {
        return jniSession?.run {
            undeclareKeyExpr(keyExpr)
        } ?: throw (sessionClosedException)
    }

    /**
     * Perform a get query handling the replies through a [BlockingQueue].
     *
     * Example using the default blocking queue receiver:
     * ```java
     * try (Session session = Zenoh.open(config)) {
     *     System.out.println("Performing Get on '" + selector + "'...");
     *     BlockingQueue<Optional<Reply>> receiver = session.get(Selector.from("a/b/c"));
     *
     *     while (true) {
     *         Optional<Reply> wrapper = receiver.take();
     *         if (wrapper.isEmpty()) {
     *             break;
     *         }
     *         Reply reply = wrapper.get();
     *         System.out.println(reply);
     *     }
     * }
     * ```
     *
     * @param selector The [Selector] for the get query.
     * @param options Optional [GetOptions] to configure the get query.
     * @return A [BlockingQueue] with the received replies.
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
     * Perform a get query handling the replies through a [Handler].
     *
     * Example using a custom handler:
     * ```java
     * // Example handler that prints the replies along with a counter:
     * class GetHandler implements Handler<Reply, Void> {
     *
     *     private Int counter = 0;
     *
     *     @Override
     *     public void handle(Reply reply) {
     *         System.out.println("Reply #" + counter + ": " + reply);
     *         counter++;
     *     }
     *
     *     @Override
     *     public Void receiver() {}
     *
     *     @Override
     *     public void onClose() {}
     * }
     *
     * //...
     * try (Session session = Zenoh.open(config)) {
     *     System.out.println("Performing Get on '" + selector + "'...");
     *     session.get(Selector.from("a/b/c"), new GetHandler());
     *     //...
     * }
     * ```
     *
     * @param R The type of the [handler]'s receiver.
     * @param selector The [Selector] for the get query.
     * @param handler The [Handler] to handle the incoming replies.
     * @param options Optional [GetOptions] to configure the query.
     * @return The handler's receiver.
     */
    @JvmOverloads
    @Throws(ZError::class)
    fun <R> get(selector: IntoSelector, handler: Handler<Reply, R>, options: GetOptions = GetOptions()): R {
        return resolveGetWithHandler(selector, handler, options)
    }

    /**
     * Perform a get query, handling the replies with a [Callback].
     *
     * Example:
     * ```java
     * try (Session session = Zenoh.open(config)) {
     *     session.get(Selector.from("a/b/c"), reply -> System.out.println(reply));
     *     //...
     * }
     * ```
     *
     * @param selector The [Selector] for the get query.
     * @param callback The [Callback] to handle the incoming replies.
     * @param options Optional [GetOptions] to configure the query.
     */
    @JvmOverloads
    @Throws(ZError::class)
    fun get(selector: IntoSelector, callback: Callback<Reply>, options: GetOptions = GetOptions()) {
        return resolveGetWithCallback(selector, callback, options)
    }

    /**
     * Perform a put with the provided [payload] to the specified [keyExpr].
     *
     * Example:
     * ```java
     * session.put(KeyExpr.from("a/b/c"), ZBytes.from("Example payload"));
     * //...
     * ```
     *
     * @param keyExpr The [KeyExpr] for performing the put.
     * @param payload The payload to put.
     * @param options Optional [PutOptions] to configure the put.
     */
    @JvmOverloads
    @Throws(ZError::class)
    fun put(keyExpr: KeyExpr, payload: IntoZBytes, options: PutOptions = PutOptions()) {
        resolvePut(keyExpr, payload, options)
    }

    /**
     * Perform a delete operation to the specified [keyExpr].
     *
     * @param keyExpr The [KeyExpr] for performing the delete operation.
     * @param options Optional [DeleteOptions] to configure the delete operation.
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
    ): HandlerSubscriber<R> {
        return jniSession?.run {
            val subscriber = declareSubscriberWithHandler(keyExpr, handler)
            declarations.add(subscriber)
            subscriber
        } ?: throw (sessionClosedException)
    }

    @Throws(ZError::class)
    internal fun resolveSubscriberWithCallback(
        keyExpr: KeyExpr, callback: Callback<Sample>
    ): CallbackSubscriber {
        return jniSession?.run {
            val subscriber = declareSubscriberWithCallback(keyExpr, callback)
            declarations.add(subscriber)
            subscriber
        } ?: throw (sessionClosedException)
    }

    @Throws(ZError::class)
    internal fun <R> resolveQueryableWithHandler(
        keyExpr: KeyExpr, handler: Handler<Query, R>, options: QueryableOptions
    ): HandlerQueryable<R> {
        return jniSession?.run {
            val queryable = declareQueryableWithHandler(keyExpr, handler, options)
            declarations.add(queryable)
            queryable
        } ?: throw (sessionClosedException)
    }

    @Throws(ZError::class)
    internal fun resolveQueryableWithCallback(
        keyExpr: KeyExpr, callback: Callback<Query>, options: QueryableOptions
    ): CallbackQueryable {
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
