//
// Copyright (c) 2026 ZettaScale Technology
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

package io.zenoh.jni.ext

/**
 * History configuration for an advanced subscriber.
 *
 * Marshaled across JNI as a plain object; the native side reads fields by
 * name via `env.get_field(...)`. `maxSamples <= 0` and `maxAgeSeconds <= 0.0`
 * mean unlimited.
 */
data class HistoryConfig(
    val detectLatePublishers: Boolean,
    val maxSamples: Long,
    val maxAgeSeconds: Double,
)
