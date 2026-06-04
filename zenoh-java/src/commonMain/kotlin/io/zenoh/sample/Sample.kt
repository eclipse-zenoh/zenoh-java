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

import io.zenoh.ZenohType
import io.zenoh.qos.QoS
import io.zenoh.qos.CongestionControl
import io.zenoh.qos.Priority
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.bytes.Encoding
import io.zenoh.bytes.ZBytes
import org.apache.commons.net.ntp.TimeStamp

/**
 * Class representing a Zenoh Sample.
 *
 * @property keyExpr The [KeyExpr] of the sample.
 * @property payload [ZBytes] with the payload of the sample.
 * @property encoding [Encoding] of the payload.
 * @property kind The [SampleKind] of the sample.
 * @property timestamp Optional [TimeStamp].
 * @property qos The Quality of Service settings used to deliver the sample.
 * @property attachment Optional attachment.
 * @property express [QoS] express value.
 * @property congestionControl The congestion control policy.
 * @property priority The priority policy.
 */
data class Sample(
    val keyExpr: KeyExpr,
    val payload: ZBytes,
    val encoding: Encoding,
    val kind: SampleKind,
    val timestamp: TimeStamp?,
    val qos: QoS,
    val attachment: ZBytes? = null,
): ZenohType {

    val express = qos.express
    val congestionControl = qos.congestionControl
    val priority = qos.priority

    internal companion object {
        /** Repacks the flat [io.zenoh.jni.sample.Sample] decoded by zenoh-flat. */
        fun from(flat: io.zenoh.jni.sample.Sample): Sample = Sample(
            KeyExpr(flat.keyExpr),
            ZBytes(flat.payload),
            Encoding(flat.encoding),
            flat.kind.toPublic(),
            flat.timestamp?.let { TimeStamp(it.ntp64) },
            QoS(
                CongestionControl.fromJni(flat.congestionControl),
                Priority.fromJni(flat.priority),
                flat.express
            ),
            flat.attachment?.let { ZBytes(it) }
        )

        /**
         * Builds a [Sample] from the flattened leaf wires the JNI callback now
         * delivers (the native side makes one `run(...)` crossing instead of
         * building a `jni.Sample` and round-tripping it). The graph is
         * reassembled in JVM bytecode via the generated `fromParts` factory.
         */
        fun fromFlat(
            keyExprString: String,
            keyExprNative: Long,
            payload: ByteArray,
            encodingId: Int,
            encodingSchema: String?,
            kind: Int,
            timestampPresent: Boolean,
            timestampNtp64: Long,
            timestampId: ByteArray?,
            express: Boolean,
            priority: Int,
            congestionControl: Int,
            attachment: ByteArray?,
        ): Sample = Sample(
            KeyExpr(keyExprString, keyExprNative),
            ZBytes.from(payload),
            Encoding(encodingId, encodingSchema),
            io.zenoh.jni.sample.SampleKind.fromInt(kind).toPublic(),
            if (timestampPresent) TimeStamp(timestampNtp64) else null,
            QoS(
                CongestionControl.fromInt(congestionControl),
                Priority.fromInt(priority),
                express
            ),
            attachment?.let { ZBytes.from(it) }
        )
    }
}
