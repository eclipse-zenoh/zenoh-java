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

import io.zenoh.annotations.Unstable
import io.zenoh.bytes.Encoding
import io.zenoh.bytes.IntoZBytes
import io.zenoh.bytes.ZBytes
import io.zenoh.bytes.into
import io.zenoh.bytes.jniHandle
import io.zenoh.bytes.jniId
import io.zenoh.bytes.jniSchema
import io.zenoh.bytes.jniSel
import io.zenoh.config.ZenohId
import io.zenoh.exceptions.ZError
import io.zenoh.exceptions.throwZError
import io.zenoh.exceptions.throwZError0
import io.zenoh.handlers.BlockingQueueHandler
import io.zenoh.handlers.Callback
import io.zenoh.handlers.Handler
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.keyexpr.jniSel
import io.zenoh.keyexpr.jniStr
import io.zenoh.keyexpr.jniHandle
import io.zenoh.liveliness.Liveliness
import io.zenoh.pubsub.*
import io.zenoh.qos.QoS
import io.zenoh.query.*
import io.zenoh.query.Query
import io.zenoh.query.Queryable
import io.zenoh.sample.Sample
import io.zenoh.session.SessionDeclaration
import io.zenoh.session.SessionInfo
import java.lang.ref.WeakReference
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
 * Native resources held by a session (and by its declarations) that is simply forgotten are released by a
 * garbage-collection backstop once the objects become unreachable; still, explicit [close] is recommended for
 * deterministic release.
 *
 * For optimal performance and adherence to good practices, it is recommended to have only one running session, which
 * is sufficient for most use cases. You should _never_ construct one session per publisher/subscriber, as this will
 * significantly increase the size of your Zenoh network, while preventing potential locality-based optimizations.
 */
class Session private constructor(private val config: Config) : AutoCloseable {

    internal var zSession: io.zenoh.jni.session.Session? = null

    // Subscribers and Queryables that keep running despite losing references to them.
    private var strongDeclarations = mutableListOf<SessionDeclaration>()

    // Publishers and queriers that shouldn't be kept alive when losing all references to them.
    private var weakDeclarations = mutableListOf<WeakReference<SessionDeclaration>>()

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
     * Every declaration still bound to the session (subscribers, queryables, publishers, queriers, …) is undeclared
     * as part of closing it.
     */
    override fun close() {
        strongDeclarations.removeIf {
            it.undeclare()
            true
        }

        weakDeclarations.removeIf {
            it.get()?.undeclare()
            true
        }

        zSession?.close()
        zSession = null
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
     *         publisher.put(payload);
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
     *         query.reply(query.getKeyExpr(), "Example reply");
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
     *          query.reply(keyExpr, "Reply #" + counter + "!");
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
     *     var queryable = session.declareQueryable(keyExpr, query -> query.reply(keyExpr, "Example reply"));
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
     * Declare a [Querier].
     *
     * A querier allows to send queries to a queryable.
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
     *     options.setPayload("Example payload");
     *     querier.get(reply -> {...}, options);
     * }
     * ```
     *
     * @param keyExpr The [KeyExpr] for the querier.
     * @param options Optional [QuerierOptions] to configure the querier.
     * @return A [Querier] that will be undeclared on drop.
     * @throws ZError
     */
    @JvmOverloads
    @Throws(ZError::class)
    fun declareQuerier(
        keyExpr: KeyExpr,
        options: QuerierOptions = QuerierOptions()
    ): Querier {
        return resolveQuerier(keyExpr, options)
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
        val zSession = zSession ?: throw sessionClosedException
        val keyexpr = run {
            // The one place a KeyExpr carries a native handle: the wire
            // declaration attached here makes sends through this session
            // compact, so the handle is worth holding.
            KeyExpr(keyExpr, zSession.declareKeyexpr(keyExpr, throwZError))
        }
        strongDeclarations.add(keyexpr)
        return keyexpr
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
        val zSession = zSession ?: throw sessionClosedException
        val handle = keyExpr.handle
        if (handle == null || handle.isClosed()) {
            throw ZError("Attempting to undeclare a non declared key expression.")
        }
        try {
            zSession.undeclareKeyexpr(handle, throwZError)
        } finally {
            // The generated wrapper consumes the handle even when the native
            // undeclare fails (the Rust side takes it by value) — detach it
            // either way, degrading the KeyExpr to its string form instead of
            // leaving a dead handle to be selected by later operations.
            keyExpr.handle = null
        }
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
     * Perform a put with the provided [payload] to the specified [keyExpr].
     *
     * Example:
     * ```java
     * session.put(KeyExpr.from("a/b/c"), "Example payload");
     * //...
     * ```
     *
     * @param keyExpr The [KeyExpr] for performing the put.
     * @param payload The payload to put as a string.
     * @param options Optional [PutOptions] to configure the put.
     */
    @JvmOverloads
    @Throws(ZError::class)
    fun put(keyExpr: KeyExpr, payload: String, options: PutOptions = PutOptions()) {
        resolvePut(keyExpr, ZBytes.from(payload), options)
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
        return zSession == null
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
        val zSession = zSession ?: throw sessionClosedException
        val publisher = run {
            // The publisher's default encoding is set NATIVELY here, once —
            // plain puts then cross no encoding data at all (see Publisher).
            val enc = options.encoding
            val zPublisher = zSession.declarePublisher(
                keyExpr.jniSel, keyExpr.jniStr, keyExpr.cloneHandle(),
                enc.jniSel, enc.jniId, enc.jniSchema, enc.jniHandle,
                options.congestionControl.jni,
                options.priority.jni,
                options.express,
                options.reliability.jni,
                throwZError
            )
            Publisher(
                keyExpr,
                options.congestionControl,
                options.priority,
                options.encoding,
                zPublisher,
            )
        }
        weakDeclarations.add(WeakReference(publisher))
        return publisher
    }

    @Throws(ZError::class)
    internal fun <R> resolveSubscriberWithHandler(
        keyExpr: KeyExpr, handler: Handler<Sample, R>
    ): HandlerSubscriber<R> {
        val zSession = zSession ?: throw sessionClosedException
        val subscriber = run {
            val zSubscriber = zSession.declareSubscriber(
                keyExpr.jniSel, keyExpr.jniStr, keyExpr.cloneHandle(),
                sampleCallbackOf { handler.handle(it) },
                { handler.onClose() },
                throwZError
            )
            HandlerSubscriber(keyExpr, zSubscriber, handler.receiver())
        }
        strongDeclarations.add(subscriber)
        return subscriber
    }

    @Throws(ZError::class)
    internal fun resolveSubscriberWithCallback(
        keyExpr: KeyExpr, callback: Callback<Sample>
    ): CallbackSubscriber {
        val zSession = zSession ?: throw sessionClosedException
        val subscriber = run {
            val zSubscriber = zSession.declareSubscriber(
                keyExpr.jniSel, keyExpr.jniStr, keyExpr.cloneHandle(),
                sampleCallbackOf { callback.run(it) },
                { },
                throwZError
            )
            CallbackSubscriber(keyExpr, zSubscriber)
        }
        strongDeclarations.add(subscriber)
        return subscriber
    }

    @Throws(ZError::class)
    internal fun <R> resolveQueryableWithHandler(
        keyExpr: KeyExpr, handler: Handler<Query, R>, options: QueryableOptions
    ): HandlerQueryable<R> {
        val zSession = zSession ?: throw sessionClosedException
        val queryable = run {
            val zQueryable = zSession.declareQueryable(
                keyExpr.jniSel, keyExpr.jniStr, keyExpr.cloneHandle(),
                options.complete,
                queryCallbackOf { handler.handle(it) },
                { handler.onClose() },
                throwZError
            )
            HandlerQueryable(keyExpr, zQueryable, handler.receiver())
        }
        strongDeclarations.add(queryable)
        return queryable
    }

    @Throws(ZError::class)
    internal fun resolveQueryableWithCallback(
        keyExpr: KeyExpr, callback: Callback<Query>, options: QueryableOptions
    ): CallbackQueryable {
        val zSession = zSession ?: throw sessionClosedException
        val queryable = run {
            val zQueryable = zSession.declareQueryable(
                keyExpr.jniSel, keyExpr.jniStr, keyExpr.cloneHandle(),
                options.complete,
                queryCallbackOf { callback.run(it) },
                { },
                throwZError
            )
            CallbackQueryable(keyExpr, zQueryable)
        }
        strongDeclarations.add(queryable)
        return queryable
    }

    @OptIn(Unstable::class)
    private fun resolveQuerier(
        keyExpr: KeyExpr,
        options: QuerierOptions
    ): Querier {
        val zSession = zSession ?: throw sessionClosedException
        val querier = run {
            val zQuerier = zSession.declareQuerier(
                keyExpr.jniSel, keyExpr.jniStr, keyExpr.cloneHandle(),
                options.target.toFlat(),
                options.consolidationMode.toFlat(),
                options.congestionControl.jni,
                options.priority.jni,
                options.express,
                options.timeout.toMillis(),
                options.acceptReplies.toFlat(),
                throwZError
            )
            Querier(
                keyExpr,
                QoS(
                    congestionControl = options.congestionControl,
                    priority = options.priority,
                    express = options.express
                ),
                zQuerier
            )
        }
        weakDeclarations.add(WeakReference(querier))
        return querier
    }

    @Throws(ZError::class)
    internal fun <R> resolveGetWithHandler(
        selector: IntoSelector,
        handler: Handler<Reply, R>,
        options: GetOptions
    ): R {
        val zSession = zSession ?: throw sessionClosedException
        return run {
            val sel = selector.into()
            zSession.get(
                sel.keyExpr.jniSel, sel.keyExpr.jniStr, sel.keyExpr.jniHandle,
                sel.parameters?.toString(),
                options.timeout.toMillis(),
                options.target.toFlat(),
                options.consolidation.toFlat(),
                options.acceptReplies.toFlat(),
                options.qos.congestionControl.jni,
                options.qos.priority.jni,
                options.qos.express,
                options.payload?.into()?.bytes,
                options.encoding.jniSel, options.encoding.jniId, options.encoding.jniSchema, options.encoding.jniHandle,
                options.attachment?.into()?.bytes,
                replyCallbackOf { handler.handle(it) },
                { handler.onClose() },
                throwZError
            )
            handler.receiver()
        }
    }

    @Throws(ZError::class)
    internal fun resolveGetWithCallback(
        selector: IntoSelector,
        callback: Callback<Reply>,
        options: GetOptions
    ) {
        val zSession = zSession ?: throw sessionClosedException
        run {
            val sel = selector.into()
            zSession.get(
                sel.keyExpr.jniSel, sel.keyExpr.jniStr, sel.keyExpr.jniHandle,
                sel.parameters?.toString(),
                options.timeout.toMillis(),
                options.target.toFlat(),
                options.consolidation.toFlat(),
                options.acceptReplies.toFlat(),
                options.qos.congestionControl.jni,
                options.qos.priority.jni,
                options.qos.express,
                options.payload?.into()?.bytes,
                options.encoding.jniSel, options.encoding.jniId, options.encoding.jniSchema, options.encoding.jniHandle,
                options.attachment?.into()?.bytes,
                replyCallbackOf { callback.run(it) },
                { },
                throwZError
            )
        }
    }

    @Throws(ZError::class)
    internal fun resolvePut(keyExpr: KeyExpr, payload: IntoZBytes, putOptions: PutOptions) {
        val zSession = zSession ?: return
        run {
            val enc = putOptions.encoding
            zSession.put(
                keyExpr.jniSel, keyExpr.jniStr, keyExpr.jniHandle,
                payload.into().bytes,
                enc.jniSel, enc.jniId, enc.jniSchema, enc.jniHandle,
                putOptions.congestionControl.jni,
                putOptions.priority.jni,
                putOptions.express,
                putOptions.attachment?.into()?.bytes,
                putOptions.reliability.jni,
                throwZError
            )
        }
    }

    @Throws(ZError::class)
    internal fun resolveDelete(keyExpr: KeyExpr, deleteOptions: DeleteOptions) {
        val zSession = zSession ?: return
        run {
            zSession.delete(
                keyExpr.jniSel, keyExpr.jniStr, keyExpr.jniHandle,
                deleteOptions.congestionControl.jni,
                deleteOptions.priority.jni,
                deleteOptions.express,
                deleteOptions.attachment?.into()?.bytes,
                deleteOptions.reliability.jni,
                throwZError
            )
        }
    }

    @Throws(ZError::class)
    internal fun zid(): ZenohId {
        val zSession = zSession ?: throw sessionClosedException
        return ZenohId(zSession.getZid(throwZError0))
    }

    @Throws(ZError::class)
    internal fun getPeersId(): List<ZenohId> {
        val zSession = zSession ?: throw sessionClosedException
        // `io.zenoh.jni.config.ZenohId` is a value class, so the native fn
        // returns `List<io.zenoh.jni.config.ZenohId>` directly; wrap each as ZenohId.
        return zSession.getPeersZid(throwZError0).map { ZenohId(it) }
    }

    @Throws(ZError::class)
    internal fun getRoutersId(): List<ZenohId> {
        val zSession = zSession ?: throw sessionClosedException
        return zSession.getRoutersZid(throwZError0).map { ZenohId(it) }
    }

    /** Launches the session, returning the [Session] on success. */
    @Throws(ZError::class)
    private fun launch(): Session {
        this.zSession = io.zenoh.jni.session.Session.open(
            config.zConfig.newClone(throwZError0),
            throwZError,
        )
        return this
    }
}
