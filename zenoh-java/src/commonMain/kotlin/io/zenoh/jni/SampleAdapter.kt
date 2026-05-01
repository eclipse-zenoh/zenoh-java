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

package io.zenoh.jni

import io.zenoh.bytes.Encoding
import io.zenoh.bytes.into
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.qos.CongestionControl
import io.zenoh.qos.Priority
import io.zenoh.qos.QoS
import io.zenoh.sample.SampleKind
import io.zenoh.sample.Sample as PublicSample
import org.apache.commons.net.ntp.TimeStamp

/**
 * Convert the auto-generated flat JNI [Sample] into the public
 * [io.zenoh.sample.Sample]. Widens the `Long` ordinals back to the
 * corresponding Kotlin enums and rebuilds the [QoS] / [TimeStamp].
 */
internal fun Sample.toPublic(): PublicSample {
    val timestamp = if (timestampIsValid) TimeStamp(timestampNtp64) else null
    return PublicSample(
        KeyExpr(keyExpr, null),
        payload.into(),
        Encoding(encodingId.toInt(), schema = encodingSchema),
        SampleKind.fromInt(kind.toInt()),
        timestamp,
        QoS(
            CongestionControl.fromInt(congestionControl.toInt()),
            Priority.fromInt(priority.toInt()),
            express,
        ),
        attachment?.into(),
    )
}
