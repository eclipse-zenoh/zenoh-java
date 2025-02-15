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
import io.zenoh.qos.*

/**
 * Options for the publisher.
 *
 * @param reliability The desired reliability.
 * @param encoding The encoding of the payload.
 * @property express [QoS] express value.
 * @property congestionControl The congestion control policy.
 * @property priority The priority policy.
 */
data class PublisherOptions(var reliability: Reliability = Reliability.RELIABLE,
                            var encoding: Encoding = Encoding.defaultEncoding(),
                            var express: Boolean = QoS.defaultPush.express,
                            var congestionControl: CongestionControl = QoS.defaultPush.congestionControl,
                            var priority: Priority = QoS.defaultPush.priority)
