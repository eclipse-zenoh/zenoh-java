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
 * is decomposed natively — `ZSample`, `ZQuery`, `ZHello` and `ZReply` arrive
 * as their leaves in ONE JNI crossing (no per-field accessor calls) and the
 * SDK object graph is built from them via [Sample.fromParts] /
 * [Query.fromParts] / the [Hello] constructor. `ZQuery` additionally delivers
 * its owned handle as the final leaf so the [Query] can reply. `ZReply` is a
 * sum type decomposed as a product: both arms' leaves are always in the
 * signature and the not-taken arm's are null — `isOk` discriminates.
 */

internal fun sampleCallbackOf(
    f: (Sample) -> Unit
): (io.zenoh.jni.keyexpr.ZKeyExpr, String, ByteArray, String, Int, Long?, Boolean, Int, Int, ByteArray?, Int, io.zenoh.jni.config.ZZenohId?, Int, Long) -> Unit =
    { keH, keStr, payload, encStr, kindInt, ntp64, express, prioInt, ccInt, attach, reliabilityInt, sourceZid, sourceEid, sourceSn ->
        f(Sample.fromParts(keH, keStr, payload, encStr, kindInt, ntp64, express, prioInt, ccInt, attach, reliabilityInt, sourceZid, sourceEid, sourceSn))
    }

internal fun queryCallbackOf(
    f: (Query) -> Unit
): (io.zenoh.jni.keyexpr.ZKeyExpr, String, String, ByteArray?, String?, ByteArray?, Int, io.zenoh.jni.query.ZQuery) -> Unit =
    { keH, keStr, parameters, payload, encStr, attach, acceptsReplies, zq ->
        // The decomposed leaves — including the cloned `keH` key-expr handle and
        // the owned `zq` query handle — are folded into the SDK [Query]. Unlike
        // the decomposed read-only types (Sample/Hello), the query OWNS `zq` and
        // is NOT closed here: it may be retained beyond this callback (e.g. put
        // on a channel by a queue handler) and replied to later. The native query
        // is dropped when it is replied to (see [Query.reply]) or when [Query] is
        // closed — that drop is what finalizes the querier's get.
        f(Query.fromParts(keH, keStr, parameters, payload, encStr, attach, acceptsReplies, zq))
    }

internal fun replyCallbackOf(
    f: (Reply) -> Unit
): (io.zenoh.jni.config.ZZenohId?, Int, Boolean, io.zenoh.jni.keyexpr.ZKeyExpr?, String?, ByteArray?, String?, Int?, Long?, Boolean?, Int?, Int?, ByteArray?, Int?, io.zenoh.jni.config.ZZenohId?, Int?, Long?, ByteArray?, String?) -> Unit =
    { zid, eid, isOk, keH, keStr, payload, encStr, kindInt, ntp64, express, prioInt, ccInt, attach, reliabilityInt, sourceZid, sourceEid, sourceSn, errPayload, errEncStr ->
        val replierId = zid?.let { EntityGlobalId(ZenohId(it), eid.toUInt()) }
        f(
            if (isOk) {
                Reply.Success(
                    replierId,
                    Sample.fromParts(keH!!, keStr!!, payload!!, encStr!!, kindInt!!, ntp64, express!!, prioInt!!, ccInt!!, attach, reliabilityInt!!, sourceZid, sourceEid!!, sourceSn!!)
                )
            } else {
                Reply.Error(
                    replierId,
                    ZBytes(errPayload!!),
                    errEncStr?.let { Encoding(it) } ?: Encoding.defaultEncoding()
                )
            }
        )
    }

internal fun helloCallbackOf(
    f: (Hello) -> Unit
): (Int, io.zenoh.jni.config.ZZenohId, List<String>) -> Unit =
    { whatamiInt, zid, locators ->
        f(Hello(WhatAmI.fromJni(io.zenoh.jni.config.WhatAmI.fromInt(whatamiInt)), ZenohId(zid), locators))
    }
