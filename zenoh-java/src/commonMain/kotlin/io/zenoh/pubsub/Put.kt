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

import io.zenoh.Resolvable
import io.zenoh.Session
import io.zenoh.bytes.Encoding
import io.zenoh.bytes.IntoZBytes
import io.zenoh.exceptions.ZError
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.qos.QoS
import io.zenoh.value.Value

/**
 * Put operation.
 *
 * A put puts a [io.zenoh.sample.Sample] into the specified key expression.
 *
 * Example:
 * ```java
 * try (Session session = Session.open()) {
 *     try (KeyExpr keyExpr = KeyExpr.tryFrom("demo/example/zenoh-java-put")) {
 *         String value = "Put from Java!";
 *         session.put(keyExpr, value)
 *             .congestionControl(CongestionControl.BLOCK)
 *             .priority(Priority.REALTIME)
 *             .kind(SampleKind.PUT)
 *             .res();
 *         System.out.println("Putting Data ('" + keyExpr + "': '" + value + "')...");
 *     }
 * }
 * ```
 *
 * This class is an open class for the sake of the [Delete] operation, which is a special case of [Put] operation.
 *
 * @property keyExpr The [KeyExpr] to which the put operation will be performed.
 * @property value The [Value] to put.
 * @property qos The [QoS] configuration.
 * @property attachment An optional user attachment.
 */
class Put private constructor(
    val keyExpr: KeyExpr,
    val payload: IntoZBytes,
    val encoding: Encoding?,
    val qos: QoS,
    val attachment: IntoZBytes?
) {

    companion object {

        /**
         * Creates a bew [Builder] associated to the specified [session] and [keyExpr].
         *
         * @param session The [Session] from which the put will be performed.
         * @param keyExpr The [KeyExpr] upon which the put will be performed.
         * @param value The [Value] to put.
         * @return A [Put] operation [Builder].
         */
        internal fun newBuilder(session: Session, keyExpr: KeyExpr, payload: IntoZBytes): Builder {
            return Builder(session, keyExpr, payload)
        }
    }

    /**
     * Builder to construct a [Put] operation.
     *
     * @property session The [Session] from which the put operation will be performed.
     * @property keyExpr The [KeyExpr] upon which the put operation will be performed.
     * @property value The [Value] to put.
     * @constructor Create a [Put] builder.
     */
    class Builder internal constructor(
        private val session: Session,
        private val keyExpr: KeyExpr,
        private val payload: IntoZBytes,
    ): Resolvable<Unit> {

        private var attachment: IntoZBytes? = null
        private var qos = QoS.default()
        private var encoding: Encoding? = null

        /** Change the [Encoding] of the written data. */
        fun encoding(encoding: Encoding) = apply {
            this.encoding = encoding
        }

        fun qos(qos: QoS) {
            this.qos = qos
        }

        /** Set an attachment to the put operation. */
        fun attachment(attachment: IntoZBytes) = apply { this.attachment = attachment }

        /** Resolves the put operation, returning a [Result]. */
        @Throws(ZError::class)
        override fun res() {
            val put = Put(keyExpr, payload, encoding, qos, attachment)
            session.run { resolvePut(keyExpr, put) }
        }
    }
}
