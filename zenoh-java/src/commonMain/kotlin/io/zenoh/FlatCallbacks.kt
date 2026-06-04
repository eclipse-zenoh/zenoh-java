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

package io.zenoh

import io.zenoh.config.WhatAmI
import io.zenoh.config.ZenohId
import io.zenoh.query.Query
import io.zenoh.query.Reply
import io.zenoh.sample.Sample
import io.zenoh.scouting.Hello

/**
 * Adapters from the *flattened* JNI callback fun-interfaces (the native side
 * delivers each `Sample`/`Query`/`Reply`/`Hello` as its leaf wires in a single
 * `run(...)` crossing — no intermediate `jni.<Struct>` object, no round-trip)
 * to a plain `(SdkType) -> Unit`. The leaf wires are reassembled into the
 * SDK object graph in JVM bytecode here, so call sites stay one-liners. The
 * long parameter lists (matching the generated `run`/`fromParts` order) live
 * here only.
 */

internal fun sampleCallbackOf(f: (Sample) -> Unit): io.zenoh.jni.callbacks.SampleCallback =
    io.zenoh.jni.callbacks.SampleCallback {
        keyExprString, keyExprNative, payload, encodingId, encodingSchema, kind,
        timestampPresent, timestampNtp64, timestampId, express, priority, congestionControl,
        attachment ->
        f(
            Sample.fromFlat(
                keyExprString, keyExprNative, payload, encodingId, encodingSchema, kind,
                timestampPresent, timestampNtp64, timestampId, express, priority,
                congestionControl, attachment
            )
        )
    }

internal fun queryCallbackOf(f: (Query) -> Unit): io.zenoh.jni.callbacks.QueryCallback =
    io.zenoh.jni.callbacks.QueryCallback {
        keyExprString, keyExprNative, parameters, payload, encodingPresent, encodingId,
        encodingSchema, attachment, acceptsReplies, query ->
        f(
            Query.fromFlat(
                keyExprString, keyExprNative, parameters, payload, encodingPresent, encodingId,
                encodingSchema, attachment, acceptsReplies, query
            )
        )
    }

internal fun replyCallbackOf(f: (Reply) -> Unit): io.zenoh.jni.callbacks.ReplyCallback =
    io.zenoh.jni.callbacks.ReplyCallback {
        replierZid, replierEid, samplePresent, sampleKeyExprString, sampleKeyExprNative,
        samplePayload, sampleEncodingId, sampleEncodingSchema, sampleKind, sampleTimestampPresent,
        sampleTimestampNtp64, sampleTimestampId, sampleExpress, samplePriority,
        sampleCongestionControl, sampleAttachment, errorPayload, errorEncodingPresent,
        errorEncodingId, errorEncodingSchema ->
        f(
            Reply.fromFlat(
                replierZid, replierEid, samplePresent, sampleKeyExprString, sampleKeyExprNative,
                samplePayload, sampleEncodingId, sampleEncodingSchema, sampleKind,
                sampleTimestampPresent, sampleTimestampNtp64, sampleTimestampId, sampleExpress,
                samplePriority, sampleCongestionControl, sampleAttachment, errorPayload,
                errorEncodingPresent, errorEncodingId, errorEncodingSchema
            )
        )
    }

internal fun helloCallbackOf(f: (Hello) -> Unit): io.zenoh.jni.callbacks.HelloCallback =
    io.zenoh.jni.callbacks.HelloCallback { whatami, zid, locators ->
        val jniHello = io.zenoh.jni.scouting.Hello.fromParts(whatami, zid, locators)
        f(Hello(WhatAmI.fromJni(jniHello.whatami), ZenohId(jniHello.zid), jniHello.locators))
    }
