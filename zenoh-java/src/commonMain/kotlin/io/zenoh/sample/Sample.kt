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
         * Decodes a native `ZSample` handle into an SDK [Sample] using the raw
         * `z_sample_*` accessors. Owned sub-handles (keyExpr, payload, encoding,
         * timestamp, attachment) are read and the transient ones freed; the
         * key-expression handle is retained by the resulting [KeyExpr].
         */
        fun from(zs: io.zenoh.jni.sample.ZSample): Sample {
            // Output expansion: the native layer decomposes the sample's
            // key-expr into BOTH its handle and string form, delivered to this
            // builder in one JNI crossing (no second `zKeyexprToString` call).
            val keyExpr = io.zenoh.jni.sample.zSampleKeyExpr(zs) { flat, str -> KeyExpr(flat, str) }
            val payload = ZBytes.fromHandle(io.zenoh.jni.sample.zSamplePayload(zs))
            val encoding = Encoding.fromHandle(io.zenoh.jni.sample.zSampleEncoding(zs))
            val kind = io.zenoh.jni.sample.zSampleKind(zs).toPublic()
            // convert_output: the native layer extracts the timestamp's NTP64
            // value and **returns** it directly (Long?, no callback) — null when
            // the sample has no timestamp.
            val timestamp = io.zenoh.jni.sample.zSampleTimestamp(zs)?.let { TimeStamp(it) }
            val qos = QoS(
                CongestionControl.fromJni(io.zenoh.jni.sample.zSampleCongestionControl(zs)),
                Priority.fromJni(io.zenoh.jni.sample.zSamplePriority(zs)),
                io.zenoh.jni.sample.zSampleExpress(zs)
            )
            // convert_output: the attachment bytes are returned directly
            // (ByteArray?, no callback); null when absent.
            val attachment = io.zenoh.jni.sample.zSampleAttachment(zs)?.let { ZBytes(it) }
            return Sample(keyExpr, payload, encoding, kind, timestamp, qos, attachment)
        }
    }
}
