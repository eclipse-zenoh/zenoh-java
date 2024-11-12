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
import io.zenoh.bytes.ZBytes
import io.zenoh.exceptions.ZError
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.qos.CongestionControl
import io.zenoh.qos.Priority
import io.zenoh.qos.QoS
import io.zenoh.qos.Reliability

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
    val payload: ZBytes,
    val encoding: Encoding,
    val qos: QoS,
    val reliability: Reliability,
    val attachment: IntoZBytes?
) {

    companion object {

        /**
         * Creates a bew [Builder] associated to the specified [session] and [keyExpr].
         *
         * @param session The [Session] from which the put will be performed.
         * @param keyExpr The [KeyExpr] upon which the put will be performed.
         * // todo
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

        private var qosBuilder = QoS.Builder()
        private var attachment: IntoZBytes? = null
        private var encoding: Encoding? = null
        private var reliability: Reliability = Reliability.RELIABLE

        /** Change the [Encoding] of the written data. */
        fun encoding(encoding: Encoding) = apply {
            this.encoding = encoding
        }

        /**
         * Sets the express flag. If true, the reply won't be batched in order to reduce the latency.
         */
        fun express(express: Boolean) = apply { qosBuilder.express(express) }

        /**
         * Sets the [Priority] of the reply.
         */
        fun priority(priority: Priority) = apply { qosBuilder.priority(priority) }

        /**
         * Sets the [CongestionControl] of the reply.
         *
         * @param congestionControl
         */
        fun congestionControl(congestionControl: CongestionControl) =
            apply { qosBuilder.congestionControl(congestionControl) }

        /**
         * The [Reliability] wished to be obtained from the network.
         */
        fun reliability(reliability: Reliability) = apply { this.reliability = reliability }

        /** Set an attachment to the put operation. */
        fun attachment(attachment: IntoZBytes) = apply { this.attachment = attachment }

        /** Resolves the put operation, returning a [Result]. */
        @Throws(ZError::class)
        override fun res() {
            // TODO: rename res() to put()
            val put = Put(keyExpr, payload.into(), encoding ?: Encoding.default(), qosBuilder.build(), reliability, attachment)
            session.run { resolvePut(keyExpr, put) }
        }
    }
}
