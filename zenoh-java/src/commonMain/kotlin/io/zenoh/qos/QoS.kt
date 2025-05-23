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

package io.zenoh.qos

/**
 * Quality of service settings used to send zenoh message.
 *
 * @property congestionControl [CongestionControl] policy used for the message.
 * @property priority [Priority] policy used for the message.
 * @property express If true, the message is not batched in order to reduce the latency.
 */
data class QoS (
    var congestionControl: CongestionControl = CongestionControl.DROP,
    var priority: Priority = Priority.DATA,
    var express: Boolean = false
) {

    companion object {
        internal val defaultPush = QoS(CongestionControl.DROP, Priority.DATA, false)
        internal val defaultRequest = QoS(CongestionControl.BLOCK, Priority.DATA, false)
        internal val defaultResponse = QoS(CongestionControl.BLOCK, Priority.DATA, false)
    }
}
