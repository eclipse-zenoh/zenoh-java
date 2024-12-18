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

import io.zenoh.bytes.IntoZBytes
import io.zenoh.qos.QoS
import io.zenoh.qos.Reliability

/**
 * Options for delete operations.
 *
 * @param qos The [QoS] (Quality of Service) desired.
 * @param reliability The [Reliability] desired.
 * @param attachment Optional attachment for the delete operation.
 */
data class DeleteOptions(
    var qos: QoS = QoS.defaultQoS(),
    var reliability: Reliability = Reliability.RELIABLE,
    var attachment: IntoZBytes? = null
)
