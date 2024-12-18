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
import io.zenoh.qos.*

/**
 * Options for the PUT operations.
 *
 * @param encoding The encoding of the payload.
 * @param reliability The desired reliability.
 * @param attachment Optional attachment.
 * @param qos The quality of service desired.
 */
data class PutOptions(
    var encoding: Encoding? = null,
    var qos: QoS = QoS.defaultQoS(),
    var reliability: Reliability = Reliability.RELIABLE,
    var attachment: IntoZBytes? = null
) {
    /**
     * Get the congestion control.
     */
    fun getCongestionControl(): CongestionControl {
        return this.qos.congestionControl
    }

    /**
     * Get the express value of the QoS.
     */
    fun getExpress(): Boolean {
        return this.qos.express
    }

    /**
     * Get the priority.
     */
    fun getPriority(): Priority {
        return this.qos.priority
    }

    /**
     * Set the QoS congestion control.
     */
    fun setCongestionControl(congestionControl: CongestionControl) {
        this.qos.congestionControl = congestionControl
    }

    /**
     * Set the QoS express value.
     */
    fun setExpress(express: Boolean) {
        this.qos.express = express
    }

    /**
     * Set the priority desired.
     */
    fun setPriority(priority: Priority) {
        this.qos.priority = priority
    }
}
