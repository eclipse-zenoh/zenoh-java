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
import io.zenoh.qos.CongestionControl
import io.zenoh.qos.Priority
import io.zenoh.qos.QoS
import io.zenoh.qos.Reliability

/**
 * TODO
 */
data class PutOptions(
    var encoding: Encoding? = null,
    var qos: QoS = QoS.defaultQoS(),
    var reliability: Reliability = Reliability.RELIABLE,
    var attachment: IntoZBytes? = null
) {
    fun getCongestionControl(): CongestionControl {
        return this.qos.congestionControl
    }

    fun getExpress(): Boolean {
        return this.qos.express
    }

    fun getPriority(): Priority {
        return this.qos.priority
    }

    fun setCongestionControl(congestionControl: CongestionControl) {
        this.qos.congestionControl = congestionControl
    }

    fun setExpress(express: Boolean) {
        this.qos.express = express
    }

    fun setPriority(priority: Priority) {
        this.qos.priority = priority
    }
}
