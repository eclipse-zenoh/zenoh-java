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
 * The Priority of Zenoh messages.
 *
 * A Priority is identified by a numeric value. Lower the value, higher the priority. Higher the value, lower the priority.
 *
 * - Highest priority: 1 ([REALTIME])
 * - Lowest priority: 7 ([BACKGROUND])
 */
enum class Priority(internal val jni: io.zenoh.jni.qos.Priority) {
    REALTIME(io.zenoh.jni.qos.Priority.REAL_TIME),
    INTERACTIVE_HIGH(io.zenoh.jni.qos.Priority.INTERACTIVE_HIGH),
    INTERACTIVE_LOW(io.zenoh.jni.qos.Priority.INTERACTIVE_LOW),
    DATA_HIGH(io.zenoh.jni.qos.Priority.DATA_HIGH),
    DATA(io.zenoh.jni.qos.Priority.DATA),
    DATA_LOW(io.zenoh.jni.qos.Priority.DATA_LOW),
    BACKGROUND(io.zenoh.jni.qos.Priority.BACKGROUND);

    val value: Int
        get() = jni.value

    companion object {
        fun fromInt(value: Int) = entries.first { it.value == value }

        internal fun fromJni(jni: io.zenoh.jni.qos.Priority): Priority = fromInt(jni.value)
    }
}
