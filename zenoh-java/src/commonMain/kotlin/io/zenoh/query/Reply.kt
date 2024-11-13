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

import io.zenoh.Resolvable
import io.zenoh.ZenohType
import io.zenoh.bytes.Encoding
import io.zenoh.bytes.IntoZBytes
import io.zenoh.bytes.ZBytes
import io.zenoh.config.ZenohId
import io.zenoh.exceptions.ZError
import io.zenoh.sample.Sample
import io.zenoh.sample.SampleKind
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.qos.CongestionControl
import io.zenoh.qos.Priority
import io.zenoh.qos.QoS
import org.apache.commons.net.ntp.TimeStamp

/**
 * Class to represent a Zenoh Reply to a [Get] operation and to a remote [Query].
 *
 * A reply can be either successful ([Success]) or an error ([Error]), both having different information. For instance,
 * the successful reply will contain a [Sample] while the error reply will only contain a [Value] with the error information.
 *
 * Replies can either be automatically created when receiving a remote reply after performing a [Get] (in which case the
 * [replierId] shows the id of the replier) or created through the builders while answering to a remote [Query] (in that
 * case the replier ID is automatically added by Zenoh).
 *
 * Generating a reply only makes sense within the context of a [Query], therefore builders below are meant to only
 * be accessible from [Query.reply].
 *
 * Example:
 * ```java
 * session.declareQueryable(keyExpr).with { query ->
 *     query.reply(keyExpr)
 *          .success(Value("Hello"))
 *          .timestamp(TimeStamp(Date.from(Instant.now())))
 *          .res()
 *     }.res()
 * ...
 * ```
 *
 * @property replierId: unique ID identifying the replier.
 */

sealed class Reply private constructor(val replierId: ZenohId?) : ZenohType {

    class Success internal constructor(replierId: ZenohId?, val sample: Sample) : Reply(replierId) {

        override fun toString(): String {
            return "Success(sample=$sample)"
        }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is Success) return false
            return sample == other.sample
        }

        override fun hashCode(): Int {
            return sample.hashCode()
        }

    }

    class Error internal constructor(replierId: ZenohId?, val error: ZBytes, val encoding: Encoding) :
        Reply(replierId) {

        override fun toString(): String {
            return "Error(error=$error)"
        }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is Error) return false

            return error == other.error
        }

        override fun hashCode(): Int {
            return error.hashCode()
        }
    }
}

class ReplyBuilder internal constructor(val query: Query, val keyExpr: KeyExpr, val payload: ZBytes, val kind: SampleKind): Resolvable<Unit> {

    private var encoding: Encoding = Encoding.defaultEncoding()
    private var timeStamp: TimeStamp? = null
    private var attachment: ZBytes? = null
    private var qosBuilder = QoS.Builder()


    fun encoding(encoding: Encoding) = apply { this.encoding = encoding }

    /**
     * Sets the [TimeStamp] of the replied [Sample].
     */
    fun timestamp(timeStamp: TimeStamp) = apply { this.timeStamp = timeStamp }

    /**
     * Appends an attachment to the reply.
     */
    fun attachment(attachment: IntoZBytes) = apply { this.attachment = attachment.into() }

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
     * Constructs the reply sample with the provided parameters and triggers the reply to the query.
     */
    @Throws(ZError::class)
    override fun res() {
        val sample = Sample(keyExpr, payload, encoding, kind, timeStamp, qosBuilder.build(), attachment)
        return query.resolveReply(Reply.Success(null, sample)).res()
    }
}

class ReplyErrBuilder internal constructor(val query: Query, val payload: ZBytes): Resolvable<Unit> {

    private var encoding: Encoding = Encoding.defaultEncoding()

    fun encoding(encoding: Encoding) = apply { this.encoding = encoding }

    /**
     * Constructs the reply sample with the provided parameters and triggers the reply to the query.
     */
    @Throws(ZError::class)
    override fun res() {
        return query.resolveReply(Reply.Error(null, payload, encoding)).res()
    }
}
