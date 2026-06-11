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
 * Adapters from the generated JNI callback lambdas to a plain
 * `(SdkType) -> Unit`. A callback argument whose type has a canonical output
 * is decomposed natively — `ZSample`, `ZQuery` and `ZHello` arrive as their
 * leaves in ONE JNI crossing (no per-field accessor calls) and the SDK object
 * graph is built from them via [Sample.fromParts] / [Query.fromParts] / the
 * [Hello] constructor. `ZQuery` additionally delivers its owned handle as the
 * final leaf so the [Query] can reply. `ZReply` has no canonical output yet, so
 * it still arrives as a whole opaque handle that native closes after the lambda
 * returns.
 */

internal fun sampleCallbackOf(
    f: (Sample) -> Unit
): (io.zenoh.jni.keyexpr.ZKeyExpr, String, ByteArray, String, Int, Long?, Boolean, Int, Int, ByteArray?) -> Unit =
    { keH, keStr, payload, encStr, kindInt, ntp64, express, prioInt, ccInt, attach ->
        f(Sample.fromParts(keH, keStr, payload, encStr, kindInt, ntp64, express, prioInt, ccInt, attach))
    }

internal fun queryCallbackOf(
    f: (Query) -> Unit
): (io.zenoh.jni.keyexpr.ZKeyExpr, String, String, ByteArray?, String?, ByteArray?, Int, io.zenoh.jni.query.ZQuery) -> Unit =
    { keH, keStr, parameters, payload, encStr, attach, acceptsReplies, zq ->
        // The decomposed leaves — including the cloned `keH` key-expr handle —
        // are folded into the SDK object graph. `zq` (the owned query handle) is
        // delivered for replying; the reply methods only *borrow* it, so we drop
        // it once the callback returns (mirrors the pre-decomposition native
        // post-callback close). Dropping the native query finalizes the reply
        // stream — without it the querier's get never completes.
        try {
            f(Query.fromParts(keH, keStr, parameters, payload, encStr, attach, acceptsReplies, zq))
        } finally {
            zq.close()
        }
    }

internal fun replyCallbackOf(f: (Reply) -> Unit): (io.zenoh.jni.query.ZReply) -> Unit =
    { zr ->
        try {
            f(Reply.from(zr))
        } finally {
            zr.close()
        }
    }

internal fun helloCallbackOf(
    f: (Hello) -> Unit
): (Int, io.zenoh.jni.config.ZZenohId, List<String>) -> Unit =
    { whatamiInt, zid, locators ->
        f(Hello(WhatAmI.fromJni(io.zenoh.jni.config.WhatAmI.fromInt(whatamiInt)), ZenohId(zid), locators))
    }
