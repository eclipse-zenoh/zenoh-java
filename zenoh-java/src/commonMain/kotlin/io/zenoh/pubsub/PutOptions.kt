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

import io.zenoh.bytes.Encoding
import io.zenoh.bytes.IntoZBytes
import io.zenoh.bytes.ZBytes
import io.zenoh.qos.*

/**
 * Options for the PUT operations.
 *
 * @param encoding The encoding of the payload.
 * @param reliability The desired reliability.
 * @param attachment Optional attachment.
 * @property express [QoS] express value.
 * @property congestionControl The congestion control policy.
 * @property priority The priority policy.
 */
data class PutOptions(
    var encoding: Encoding? = null,
    var reliability: Reliability = Reliability.RELIABLE,
    var attachment: IntoZBytes? = null,
    var express: Boolean = QoS.defaultPush.express,
    var congestionControl: CongestionControl = QoS.defaultPush.congestionControl,
    var priority: Priority = QoS.defaultPush.priority
) {
    fun setAttachment(attachment: String) = apply { this.attachment = ZBytes.from(attachment) }
}
