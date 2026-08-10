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

package io.zenoh.sample

/** The kind of sample. */
enum class SampleKind {
    PUT,
    DELETE;

    companion object {
        fun fromInt(value: Int) = entries.first { it.ordinal == value }
    }
}

internal fun io.zenoh.jni.sample.SampleKind.toPublic(): SampleKind = when (this) {
    io.zenoh.jni.sample.SampleKind.PUT -> SampleKind.PUT
    io.zenoh.jni.sample.SampleKind.DELETE -> SampleKind.DELETE
}
