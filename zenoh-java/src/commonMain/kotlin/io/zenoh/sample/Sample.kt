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
        /**
         * Builds an SDK [Sample] from the 15 leaves of the ZSample canonical
         * output — the lambda parameter list every decomposed sample delivery
         * uses (the `zReplySample` builder and the subscriber/liveliness
         * callbacks), in record order. The whole graph arrives in ONE JNI
         * crossing; the [KeyExpr] retains the delivered handle leaf, and the
         * [Encoding] retains its delivered handle (`encH`) so re-sending it
         * crosses a bare `jlong` instead of rebuilding the native value —
         * delivered ONLY for schema-carrying encodings (a preset re-sends
         * through the id arm for free, so it arrives value-only). The
         * trailing `reliability` / `source*` leaves are part of the generated
         * decomposition but are not surfaced on the public [Sample] type.
         */
        @Suppress("UNUSED_PARAMETER")
        fun fromParts(
            keH: io.zenoh.jni.keyexpr.KeyExpr,
            payloadH: io.zenoh.jni.bytes.ZBytes,
            encId: Int,
            encSchema: String?,
            encH: io.zenoh.jni.bytes.Encoding?,
            kindInt: Int,
            ntp64: Long?,
            express: Boolean,
            prioInt: Int,
            ccInt: Int,
            attachH: io.zenoh.jni.bytes.ZBytes?,
            reliabilityInt: Int,
            sourceZid: io.zenoh.jni.config.ZenohId?,
            sourceEid: Int,
            sourceSn: Long,
        ): Sample = Sample(
            KeyExpr(keH),
            ZBytes.fromHandle(payloadH),
            Encoding(encId, encSchema, encH),
            io.zenoh.jni.sample.SampleKind.fromInt(kindInt).toPublic(),
            ntp64?.let { TimeStamp(it) },
            QoS(
                CongestionControl.fromJni(io.zenoh.jni.qos.CongestionControl.fromInt(ccInt)),
                Priority.fromJni(io.zenoh.jni.qos.Priority.fromInt(prioInt)),
                express
            ),
            attachH?.let { ZBytes.fromHandle(it) }
        )
    }
}
