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
import io.zenoh.time.Timestamp

/**
 * Class representing a Zenoh Sample.
 *
 * @property keyExpr The [KeyExpr] of the sample.
 * @property payload [ZBytes] with the payload of the sample.
 * @property encoding [Encoding] of the payload.
 * @property kind The [SampleKind] of the sample.
 * @property timestamp Optional [Timestamp].
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
    val timestamp: Timestamp?,
    val qos: QoS,
    val attachment: ZBytes? = null,
): ZenohType {

    val express = qos.express
    val congestionControl = qos.congestionControl
    val priority = qos.priority

    internal companion object {
        /**
         * Builds an SDK [Sample] from the 12 leaves of the ZSample canonical
         * output — the lambda parameter list every decomposed sample delivery
         * uses (the `zReplySample` builder and the subscriber/liveliness
         * callbacks), in record order. The whole graph arrives in ONE JNI
         * crossing; the [KeyExpr] is string-backed (a received keyexpr never
         * carries a wire declaration, so a native handle would buy nothing),
         * and the [Encoding] is value-only `(id, schema?)` — a delivered
         * native handle measurably cost more per receive than the
         * schema-string re-decode it saved on the resent fraction. The
         * trailing `reliability` / `sourceInfo` leaves are part of the
         * generated decomposition but are not surfaced on the public [Sample]
         * type.
         */
        @Suppress("UNUSED_PARAMETER")
        fun fromParts(
            keStr: String,
            payloadH: io.zenoh.jni.bytes.ZBytes,
            encId: Int,
            encSchema: ByteArray?,
            kindInt: Int,
            timestamp: io.zenoh.jni.time.Timestamp?,
            express: Boolean,
            prioInt: Int,
            ccInt: Int,
            attachH: io.zenoh.jni.bytes.ZBytes?,
            reliabilityInt: Int,
            sourceInfo: io.zenoh.jni.sample.SourceInfo?,
        ): Sample = Sample(
            KeyExpr(keStr),
            ZBytes.fromHandle(payloadH),
            // A schema is raw bytes on the wire and zenoh does not require it
            // to be UTF-8; this SDK's Encoding carries a String, so a schema
            // that is not valid UTF-8 decodes lossily rather than throwing on
            // a received message.
            Encoding(encId, encSchema?.toString(Charsets.UTF_8)),
            io.zenoh.jni.sample.SampleKind.fromInt(kindInt).toPublic(),
            timestamp?.let { Timestamp.fromJni(it) },
            QoS(
                CongestionControl.fromJni(io.zenoh.jni.qos.CongestionControl.fromInt(ccInt)),
                Priority.fromJni(io.zenoh.jni.qos.Priority.fromInt(prioInt)),
                express
            ),
            attachH?.let { ZBytes.fromHandle(it) }
        )
    }
}
