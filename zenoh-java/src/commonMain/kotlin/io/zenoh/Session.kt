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

import io.zenoh.bytes.Encoding
import io.zenoh.bytes.IntoZBytes
import io.zenoh.exceptions.ZError
import io.zenoh.handlers.Callback
import io.zenoh.jni.JNISession
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.qos.QoS
import io.zenoh.pubsub.Delete
import io.zenoh.pubsub.Publisher
import io.zenoh.pubsub.Put
import io.zenoh.query.*
import io.zenoh.query.Query
import io.zenoh.query.Queryable
import io.zenoh.sample.Sample
import io.zenoh.query.Selector
import io.zenoh.qos.Reliability
import io.zenoh.pubsub.Subscriber
import io.zenoh.value.Value
import java.time.Duration
import java.util.*
import java.util.concurrent.BlockingQueue

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

    private var jniSession: JNISession? = JNISession()

    private var declarations = mutableListOf<SessionDeclaration>()

    companion object {

        private val sessionClosedException = ZError("Session is closed.")

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
    @Throws(ZError::class)
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
     * Example:
     * ```java
     * try (Session session = Session.open()) {
     *     try (KeyExpr keyExpr = KeyExpr.tryFrom("demo/example/greeting")) {
     *         try (Publisher publisher = session.declarePublisher(keyExpr)
     *             .priority(Priority.REALTIME)
     *             .congestionControl(CongestionControl.DROP)
     *             .res()) {
     *                 int idx = 0;
     *                 while (true) {
     *                     String payload = "Hello for the " + idx + "th time!";
     *                     publisher.put(payload).res();
     *                     Thread.sleep(1000);
     *                     idx++;
     *             }
     *         }
     *     }
     * }
     * ```
     *
     * @param keyExpr The [KeyExpr] the publisher will be associated to.
     * @return A resolvable [Publisher.Builder]
     */
    fun declarePublisher(keyExpr: KeyExpr): Publisher.Builder = Publisher.Builder(this, keyExpr)

    /**
     * Declare a [Subscriber] on the session.
     *
     * The default receiver is a [BlockingQueue], but can be changed with the [Subscriber.Builder.callback] functions.
     *
     * Example:
     *
     * ```java
     * try (Session session = Session.open()) {
     *     try (KeyExpr keyExpr = KeyExpr.tryFrom("demo/example/sub")) {
     *         try (Subscriber<BlockingQueue<Optional<Sample>>> subscriber = session.declareSubscriber(keyExpr).res()) {
     *             BlockingQueue<Optional<Sample>> receiver = subscriber.getReceiver();
     *             assert receiver != null;
     *             while (true) {
     *                 Optional<Sample> sample = receiver.take();
     *                 if (sample.isEmpty()) {
     *                     break;
     *                 }
     *                 System.out.println(sample.get());
     *             }
     *         }
     *     }
     * }
     * ```
     *
     * @param keyExpr The [KeyExpr] the subscriber will be associated to.
     * @return A [Subscriber.Builder] with a [BlockingQueue] receiver.
     */
    fun declareSubscriber(keyExpr: KeyExpr): Subscriber.Builder<BlockingQueue<Optional<Sample>>> = Subscriber.newBuilder(this, keyExpr)

    /**
     * Declare a [Queryable] on the session.
     *
     * The default receiver is a [BlockingQueue], but can be changed with the [Queryable.Builder.callback] functions.
     *
     * Example:
     * ```java
     * try (Session session = Session.open()) {
     *     try (KeyExpr keyExpr = KeyExpr.tryFrom("demo/example/greeting")) {
     *         System.out.println("Declaring Queryable");
     *         try (Queryable<BlockingQueue<Optional<Query>>> queryable = session.declareQueryable(keyExpr).res()) {
     *             BlockingQueue<Optional<Query>> receiver = queryable.getReceiver();
     *             while (true) {
     *                 Optional<Query> wrapper = receiver.take();
     *                 if (wrapper.isEmpty()) {
     *                     break;
     *                 }
     *                 Query query = wrapper.get();
     *                 System.out.println("Received query at " + query.getSelector());
     *                 query.reply(keyExpr)
     *                     .success("Hello!")
     *                     .withKind(SampleKind.PUT)
     *                     .withTimeStamp(TimeStamp.getCurrentTime())
     *                     .res();
     *             }
     *         }
     *     }
     * }
     * ```
     *
     * @param keyExpr The [KeyExpr] the queryable will be associated to.
     * @return A [Queryable.Builder] with a [BlockingQueue] receiver.
     */
    fun declareQueryable(keyExpr: KeyExpr): Queryable.Builder<BlockingQueue<Optional<Query>>> = Queryable.newBuilder(this, keyExpr)

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
    fun declareKeyExpr(keyExpr: String): Resolvable<KeyExpr> = Resolvable {
        return@Resolvable jniSession?.run {
            declareKeyExpr(keyExpr)
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
     * try (Session session = Session.open()) {
     *     try (Selector selector = Selector.tryFrom("demo/java/example")) {
     *          session.get(selector)
     *              .consolidation(ConsolidationMode.NONE)
     *              .withValue("Get value example")
     *              .with(reply -> System.out.println("Received reply " + reply))
     *              .res()
     *     }
     * }
     * ```
     *
     * @param selector The [KeyExpr] to be used for the get operation.
     * @return a resolvable [Get.Builder] with a [BlockingQueue] receiver.
     */
    fun get(selector: Selector): Get.Builder<BlockingQueue<Optional<Reply>>> = Get.newBuilder(this, selector)

    /**
     * Declare a [Get] with a [BlockingQueue] receiver as default.
     *
     * ```java
     * try (Session session = Session.open()) {
     *     try (KeyExpr keyExpr = KeyExpr.tryFrom("demo/java/example")) {
     *          session.get(keyExpr)
     *              .consolidation(ConsolidationMode.NONE)
     *              .withValue("Get value example")
     *              .with(reply -> System.out.println("Received reply " + reply))
     *              .res()
     *     }
     * }
     * ```
     *
     * @param keyExpr The [KeyExpr] to be used for the get operation.
     * @return a resolvable [Get.Builder] with a [BlockingQueue] receiver.
     */
    fun get(keyExpr: KeyExpr): Get.Builder<BlockingQueue<Optional<Reply>>> = Get.newBuilder(this, Selector(keyExpr))

    /**
     * Declare a [Put] with the provided value on the specified key expression.
     *
     * Example:
     * ```java
     * try (Session session = Session.open()) {
     *     try (KeyExpr keyExpr = KeyExpr.tryFrom("demo/example/greeting")) {
     *         session.put(keyExpr, Value("Hello!"))
     *             .congestionControl(CongestionControl.BLOCK)
     *             .priority(Priority.REALTIME)
     *             .kind(SampleKind.PUT)
     *             .res();
     *         System.out.println("Put 'Hello' on " + keyExpr + ".");
     *     }
     * }
     * ```
     *
     * @param keyExpr The [KeyExpr] to be used for the put operation.
     * @param value The [Value] to be put.
     * @return A resolvable [Put.Builder].
     */
    fun put(keyExpr: KeyExpr, value: Value): Put.Builder = Put.newBuilder(this, keyExpr, value)

    /**
     * Declare a [Put] with the provided value on the specified key expression.
     *
     * Example:
     * ```java
     * try (Session session = Session.open()) {
     *     try (KeyExpr keyExpr = KeyExpr.tryFrom("demo/example/greeting")) {
     *         session.put(keyExpr, "Hello!")
     *             .congestionControl(CongestionControl.BLOCK)
     *             .priority(Priority.REALTIME)
     *             .kind(SampleKind.PUT)
     *             .res();
     *         System.out.println("Put 'Hello' on " + keyExpr + ".");
     *     }
     * }
     * ```
     *
     * @param keyExpr The [KeyExpr] to be used for the put operation.
     * @param message The message to be put.
     * @return A resolvable [Put.Builder].
     */
    fun put(keyExpr: KeyExpr, message: String): Put.Builder = Put.newBuilder(this, keyExpr, Value(message))

    /**
     * Declare a [Delete].
     *
     * Example:
     *
     * ```java
     * try (Session session = Session.open()) {
     *     try (KeyExpr keyExpr = KeyExpr.tryFrom("demo/example")) {
     *         session.delete(keyExpr).res();
     *         System.out.println("Performed delete on " + keyExpr + ".");
     *     }
     * }
     * ```
     *
     * @param keyExpr The [KeyExpr] to be used for the delete operation.
     * @return a resolvable [Delete.Builder].
     */
    fun delete(keyExpr: KeyExpr): Delete.Builder = Delete.newBuilder(this, keyExpr)

    /** Returns if session is open or has been closed. */
    fun isOpen(): Boolean {
        return jniSession != null
    }

    @Throws(ZError::class)
    internal fun resolvePublisher(keyExpr: KeyExpr, encoding: Encoding, qos: QoS, reliability: Reliability): Publisher {
        return jniSession?.run {
            declarePublisher(keyExpr, qos, encoding, reliability)
        } ?: throw(sessionClosedException)
    }

    @Throws(ZError::class)
    internal fun <R> resolveSubscriber(
        keyExpr: KeyExpr, callback: Callback<Sample>, onClose: () -> Unit, receiver: R
    ): Subscriber<R> {
        return jniSession?.run {
            declareSubscriber(keyExpr, callback, onClose, receiver)
        } ?: throw (sessionClosedException)
    }

    @Throws(ZError::class)
    internal fun <R> resolveQueryable(
        keyExpr: KeyExpr, callback: Callback<Query>, onClose: () -> Unit, receiver: R, complete: Boolean
    ): Queryable<R> {
        return jniSession?.run {
            declareQueryable(keyExpr, callback, onClose, receiver, complete)
        } ?: throw (sessionClosedException)
    }

    @Throws(ZError::class)
    internal fun <R> resolveGet(
        selector: Selector,
        callback: Callback<Reply>,
        onClose: () -> Unit,
        receiver: R?,
        timeout: Duration,
        target: QueryTarget,
        consolidation: ConsolidationMode,
        payload: IntoZBytes?,
        encoding: Encoding?,
        attachment: IntoZBytes?,
    ): R? {
        if (jniSession == null) {
            throw sessionClosedException
        }
        return jniSession?.performGet(selector, callback, onClose, receiver, timeout, target, consolidation, payload, encoding, attachment)
    }

    @Throws(ZError::class)
    internal fun resolvePut(keyExpr: KeyExpr, put: Put) {
        jniSession?.run { performPut(keyExpr, put) }
    }

    @Throws(ZError::class)
    internal fun resolveDelete(keyExpr: KeyExpr, delete: Delete) {
        jniSession?.run { performDelete(keyExpr, delete) }
    }

    /** Launches the session through the jni session, returning the [Session] on success. */
    @Throws(ZError::class)
    private fun launch(): Session {
        jniSession!!.open(config)
        return this
    }
}
