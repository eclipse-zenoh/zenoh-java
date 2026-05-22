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

import io.zenoh.jni.JniPriority as JniPriority

/**
 * The Priority of Zenoh messages.
 *
 * A Priority is identified by a numeric value. Lower the value, higher the priority. Higher the value, lower the priority.
 *
 * - Highest priority: 1 ([REALTIME])
 * - Lowest priority: 7 ([BACKGROUND])
 */
enum class Priority(val value: Int) {
    REALTIME(1),
    INTERACTIVE_HIGH(2),
    INTERACTIVE_LOW(3),
    DATA_HIGH(4),
    DATA(5),
    DATA_LOW(6),
    BACKGROUND(7);

    /**
     * Project this public-API priority onto the JNI-layer enum that
     * crosses the boundary. Variant-for-variant, no `Int` round-trip.
     */
    fun toJni(): JniPriority = when (this) {
        REALTIME -> JniPriority.REAL_TIME
        INTERACTIVE_HIGH -> JniPriority.INTERACTIVE_HIGH
        INTERACTIVE_LOW -> JniPriority.INTERACTIVE_LOW
        DATA_HIGH -> JniPriority.DATA_HIGH
        DATA -> JniPriority.DATA
        DATA_LOW -> JniPriority.DATA_LOW
        BACKGROUND -> JniPriority.BACKGROUND
    }

    companion object {
        fun fromInt(value: Int) = entries.first { it.value == value }

        /**
         * Lift a JNI-layer priority into the public-API enum.
         * Variant-for-variant, no `Int` round-trip.
         */
        fun fromJni(p: JniPriority): Priority = when (p) {
            JniPriority.REAL_TIME -> REALTIME
            JniPriority.INTERACTIVE_HIGH -> INTERACTIVE_HIGH
            JniPriority.INTERACTIVE_LOW -> INTERACTIVE_LOW
            JniPriority.DATA_HIGH -> DATA_HIGH
            JniPriority.DATA -> DATA
            JniPriority.DATA_LOW -> DATA_LOW
            JniPriority.BACKGROUND -> BACKGROUND
        }
    }
}
