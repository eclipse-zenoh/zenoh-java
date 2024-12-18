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

import io.zenoh.ZenohType
import io.zenoh.bytes.Encoding
import io.zenoh.bytes.ZBytes
import io.zenoh.config.ZenohId
import io.zenoh.sample.Sample
import io.zenoh.qos.CongestionControl
import io.zenoh.qos.Priority
import io.zenoh.qos.QoS
import org.apache.commons.net.ntp.TimeStamp

/**
 * Class to represent a Zenoh Reply to a remote query.
 *
 * @property replierId: unique ID identifying the replier.
 * @see Success
 * @see Error
 */
sealed class Reply private constructor(val replierId: ZenohId?) : ZenohType {

    /**
     * A Success reply.
     */
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

    /**
     * An Error reply.
     */
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

/**
 * Options for performing a [Reply] to a [Query].
 *
 * @param encoding [Encoding] of the payload of the reply.
 * @param timeStamp Optional timestamp.
 * @param attachment Optional attachment.
 * @property express [QoS] express value.
 * @property congestionControl The congestion control policy.
 * @property priority The priority policy.
 */
data class ReplyOptions(
    var encoding: Encoding = Encoding.defaultEncoding(),
    var timeStamp: TimeStamp? = null,
    var attachment: ZBytes? = null,
    var express: Boolean = QoS.defaultQoS.express,
    var congestionControl: CongestionControl = QoS.defaultQoS.congestionControl,
    var priority: Priority = QoS.defaultQoS.priority
)

/**
 * Options for performing a Reply Delete to a [Query].
 *
 * @param timeStamp Optional timestamp.
 * @param attachment Optional attachment.
 * @property express [QoS] express value.
 * @property congestionControl The congestion control policy.
 * @property priority The priority policy.
 */
data class ReplyDelOptions(
    var timeStamp: TimeStamp? = null,
    var attachment: ZBytes? = null,
    var express: Boolean = QoS.defaultQoS.express,
    var congestionControl: CongestionControl = QoS.defaultQoS.congestionControl,
    var priority: Priority = QoS.defaultQoS.priority
)


/**
 * Options for performing a Reply Err to a [Query].
 *
 * @param encoding The encoding of the error message.
 */
data class ReplyErrOptions(var encoding: Encoding = Encoding.defaultEncoding())
