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
 * Cache configuration for an advanced publisher.
 *
 * Marshaled across JNI as a plain object; the native side reads fields by
 * name via `env.get_field(...)`. The nested `RepliesConfig` is represented
 * inline (flat fields `repliesPriority`, `repliesCongestionControl`,
 * `repliesIsExpress`) to keep the JNI field-lookup simple.
 *
 * `repliesPriority` and `repliesCongestionControl` are the ordinals of the
 * corresponding zenoh enums.
 */
data class CacheConfig(
    val maxSamples: Long,
    val repliesPriority: Int,
    val repliesCongestionControl: Int,
    val repliesIsExpress: Boolean,
)
