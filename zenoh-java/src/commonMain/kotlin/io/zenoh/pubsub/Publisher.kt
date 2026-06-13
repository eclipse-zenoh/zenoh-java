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
import io.zenoh.bytes.Encoding
import io.zenoh.bytes.IntoZBytes
import io.zenoh.bytes.ZBytes
import io.zenoh.exceptions.ZError
import io.zenoh.exceptions.throwZError
import io.zenoh.jni.pubsub.ZPublisher
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.qos.CongestionControl
import io.zenoh.qos.Priority
import io.zenoh.session.SessionDeclaration
import kotlin.Throws

/**
 * A Zenoh Publisher.
 *
 * A publisher is automatically dropped when using it with the 'try-with-resources' statement (i.e. 'use' in Kotlin).
 * The session from which it was declared will also keep a reference to it and undeclare it once the session is closed.
 *
 * In order to declare a publisher, [Session.declarePublisher] must be called.
 *
 * Example:
 * ```java
 * try (Session session = Session.open()) {
 *     try (KeyExpr keyExpr = KeyExpr.tryFrom("demo/java/greeting")) {
 *         System.out.println("Declaring publisher on '" + keyExpr + "'...");
 *         try (Publisher publisher = session.declarePublisher(keyExpr)) {
 *             int i = 0;
 *             while (true) {
 *                 publisher.put("Hello for the " + i + "th time!");
 *                 Thread.sleep(1000);
 *                 i++;
 *             }
 *         }
 *     }
 * } catch (ZError | InterruptedException e) {
 *     System.out.println("Error: " + e);
 * }
 * ```
 *
 * The publisher configuration parameters can be later changed using the setter functions.
 *
 * @property keyExpr The key expression the publisher will be associated to.
 * @property encoding The encoding user by the publisher.
 */
class Publisher internal constructor(
    val keyExpr: KeyExpr,
    private var congestionControl: CongestionControl,
    private var priority: Priority,
    val encoding: Encoding,
    private var zPublisher: ZPublisher?,
) : SessionDeclaration, AutoCloseable {

    companion object {
        private val publisherNotValid = ZError("Publisher is not valid.")
    }

    /** Get the congestion control applied when routing the data. */
    fun congestionControl() = congestionControl

    /** Get the priority of the written data. */
    fun priority() = priority

    /** Performs a PUT operation on the specified [keyExpr] with the specified [payload]. */
    @Throws(ZError::class)
    fun put(payload: IntoZBytes) {
        performPut(payload, encoding, null)
    }

    /** Performs a PUT operation on the specified [keyExpr] with the specified [payload]. */
    @Throws(ZError::class)
    fun put(payload: IntoZBytes, options: PutOptions) {
        performPut(payload, options.encoding ?: this.encoding, options.attachment)
    }

    /** Performs a PUT operation on the specified [keyExpr] with the specified [payload]. */
    @Throws(ZError::class)
    fun put(payload: String) = put(ZBytes.from(payload))

    /** Performs a PUT operation on the specified [keyExpr] with the specified [payload]. */
    @Throws(ZError::class)
    fun put(payload: String, options: PutOptions) = put(ZBytes.from(payload), options)

    /**
     * Performs a DELETE operation on the specified [keyExpr]
     */
    @JvmOverloads
    @Throws(ZError::class)
    fun delete(options: DeleteOptions = DeleteOptions()) {
        val p = zPublisher ?: throw publisherNotValid
        io.zenoh.jni.pubsub.zPublisherDelete(p, options.attachment?.into()?.bytes, throwZError)
    }

    /**
     * Returns `true` if the publisher is still running.
     */
    fun isValid(): Boolean {
        return zPublisher != null
    }

    override fun close() {
        undeclare()
    }

    override fun undeclare() {
        zPublisher?.close()
        zPublisher = null
    }

    @Suppress("removal")
    protected fun finalize() {
        zPublisher?.close()
    }

    @Throws(ZError::class)
    private fun performPut(payload: IntoZBytes, encoding: Encoding, attachment: IntoZBytes?) {
        val p = zPublisher ?: throw publisherNotValid
        io.zenoh.jni.pubsub.zPublisherPut(
            p,
            payload.into().bytes,
            true,
            encoding.idForWire(),
            encoding.schemaForWire(),
            attachment?.into()?.bytes,
            throwZError,
        )
    }
}
