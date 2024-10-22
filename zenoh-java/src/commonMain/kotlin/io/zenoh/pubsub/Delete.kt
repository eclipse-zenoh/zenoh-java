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
import io.zenoh.bytes.IntoZBytes
import io.zenoh.exceptions.ZError
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.qos.CongestionControl
import io.zenoh.qos.Priority
import io.zenoh.qos.QoS
import kotlin.Throws

/**
 * Delete operation to perform on Zenoh on a key expression.
 *
 * Example:
 * ```java
 * public void deleteExample() throws ZError {
 *     System.out.println("Opening session...");
 *     try (Session session = Session.open()) {
 *         try (KeyExpr keyExpr = KeyExpr.tryFrom("demo/java/example")) {
 *             session.delete(keyExpr).res();
 *             System.out.println("Performed a delete on '" + keyExpr);
 *         }
 *     }
 * }
 * ```
 *
 * A delete operation is a special case of a Put operation, it is analogous to perform a Put with an empty value and
 * specifying the sample kind to be `DELETE`.
 */
class Delete private constructor(
    val keyExpr: KeyExpr, val qos: QoS, val attachment: IntoZBytes?
) {

    companion object {
        /**
         * Creates a new [Builder] associated with the specified [session] and [keyExpr].
         *
         * @param session The [Session] from which the Delete will be performed.
         * @param keyExpr The [KeyExpr] upon which the Delete will be performed.
         * @return A [Delete] operation [Builder].
         */
        fun newBuilder(session: Session, keyExpr: KeyExpr): Builder {
            return Builder(session, keyExpr)
        }
    }

    /**
     * Builder to construct a [Delete] operation.
     *
     * @property session The [Session] from which the Delete will be performed
     * @property keyExpr The [KeyExpr] from which the Delete will be performed
     * @constructor Create a [Delete] builder.
     */
    class Builder internal constructor(
        val session: Session,
        val keyExpr: KeyExpr,
    ) : Resolvable<Unit> {

        private var attachment: IntoZBytes? = null

        private var qos: QoS = QoS.default()

        fun attachment(attachment: IntoZBytes) {
            this.attachment = attachment
        }

        fun qos(qos: QoS) {
            this.qos = qos
        }

        /**
         * Performs a DELETE operation on the specified [keyExpr].
         *
         * A successful [Result] only states the Delete request was properly sent through the network, it doesn't mean it
         * was properly executed remotely.
         */
        @Throws(ZError::class)
        override fun res() {
            val delete = Delete(this.keyExpr, qos, attachment)
            session.resolveDelete(keyExpr, delete)
        }
    }
}