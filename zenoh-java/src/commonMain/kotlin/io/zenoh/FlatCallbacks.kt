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

import io.zenoh.bytes.Encoding
import io.zenoh.bytes.ZBytes
import io.zenoh.config.EntityGlobalId
import io.zenoh.config.WhatAmI
import io.zenoh.config.ZenohId
import io.zenoh.query.Query
import io.zenoh.query.Reply
import io.zenoh.sample.Sample
import io.zenoh.scouting.Hello

/**
 * Adapters from the generated JNI callback lambdas to a plain
 * `(SdkType) -> Unit`. A callback argument whose type has a canonical output
 * is decomposed natively — a `Sample`, `Query`, `Hello` or `Reply` arrives
 * as its leaves in ONE JNI crossing (no per-field accessor calls) and the
 * SDK object graph is built from them via [Sample.fromParts] /
 * [Query.fromParts] / the [Hello] constructor. A `Query` additionally delivers
 * its owned handle as the final leaf so the [Query] can reply. A `Reply` is a
 * sum type decomposed as a product: both arms' leaves are always in the
 * signature and the not-taken arm's are null — `isOk` discriminates.
 */

internal fun sampleCallbackOf(
    f: (Sample) -> Unit
): io.zenoh.jni.sample.SampleCallback =
    io.zenoh.jni.sample.SampleCallback { keH, payloadH, encH, encId, kindInt, ntp64, express, prioInt, ccInt, attachH, reliabilityInt, sourceZid, sourceEid, sourceSn ->
        f(Sample.fromParts(keH, payloadH, encH, encId, kindInt, ntp64, express, prioInt, ccInt, attachH, reliabilityInt, sourceZid, sourceEid, sourceSn))
    }

internal fun queryCallbackOf(
    f: (Query) -> Unit
): io.zenoh.jni.query.QueryCallback =
    io.zenoh.jni.query.QueryCallback { keH, parameters, payloadH, encH, encId, attachH, acceptsReplies, zq ->
        // The decomposed leaves — including the cloned `keH` key-expr handle and
        // the owned `zq` query handle — are folded into the SDK [Query]. Unlike
        // the decomposed read-only types (Sample/Hello), the query OWNS `zq` and
        // is NOT closed here: it may be retained beyond this callback (e.g. put
        // on a channel by a queue handler) and replied to later. The native query
        // is dropped when it is replied to (see [Query.reply]) or when [Query] is
        // closed — that drop is what finalizes the querier's get.
        f(Query.fromParts(keH, parameters, payloadH, encH, encId, attachH, acceptsReplies, zq))
    }

internal fun replyCallbackOf(
    f: (Reply) -> Unit
): io.zenoh.jni.query.ReplyCallback =
    io.zenoh.jni.query.ReplyCallback { zid, eid, isOk, keH, payloadH, encH, encId, kindInt, ntp64, express, prioInt, ccInt, attachH, reliabilityInt, sourceZid, sourceEid, sourceSn, errPayloadH, errEncH, errEncId ->
        val replierId = zid?.let { EntityGlobalId(ZenohId(it), eid.toUInt()) }
        f(
            if (isOk) {
                Reply.Success(
                    replierId,
                    Sample.fromParts(keH!!, payloadH!!, encH!!, encId!!, kindInt!!, ntp64, express!!, prioInt!!, ccInt!!, attachH, reliabilityInt!!, sourceZid, sourceEid!!, sourceSn!!)
                )
            } else {
                Reply.Error(
                    replierId,
                    ZBytes.fromHandle(errPayloadH!!),
                    errEncH?.let { Encoding.fromParts(it, errEncId!!) } ?: Encoding.defaultEncoding()
                )
            }
        )
    }

internal fun helloCallbackOf(
    f: (Hello) -> Unit
): io.zenoh.jni.scouting.HelloCallback =
    io.zenoh.jni.scouting.HelloCallback { whatamiInt, zid, locators ->
        f(Hello(WhatAmI.fromJni(io.zenoh.jni.config.WhatAmI.fromInt(whatamiInt)), ZenohId(zid), locators))
    }
