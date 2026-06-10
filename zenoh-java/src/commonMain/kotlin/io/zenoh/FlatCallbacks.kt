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
 * is decomposed natively — `ZSample` arrives as its 10 leaves in ONE JNI
 * crossing (no transient handle, no per-field accessor calls) and the SDK
 * object graph is built from them via [Sample.fromParts]. Plan-less argument
 * types (`ZQuery`/`ZReply`/`ZHello`) still arrive as a whole opaque handle;
 * native closes it after the lambda returns (a no-op when the handle was
 * consumed — [Query]'s reply methods consume `zq`, so replying keeps working).
 */

internal fun sampleCallbackOf(
    f: (Sample) -> Unit
): (io.zenoh.jni.keyexpr.ZKeyExpr, String, ByteArray, String, Int, Long?, Boolean, Int, Int, ByteArray?) -> Unit =
    { keH, keStr, payload, encStr, kindInt, ntp64, express, prioInt, ccInt, attach ->
        f(Sample.fromParts(keH, keStr, payload, encStr, kindInt, ntp64, express, prioInt, ccInt, attach))
    }

internal fun queryCallbackOf(f: (Query) -> Unit): (io.zenoh.jni.query.ZQuery) -> Unit =
    { zq ->
        // The [Query] retains `zq` (its reply methods consume it); the native
        // post-invoke close is a no-op once a reply consumed the handle.
        f(Query.from(zq))
    }

internal fun replyCallbackOf(f: (Reply) -> Unit): (io.zenoh.jni.query.ZReply) -> Unit =
    { zr ->
        try {
            f(Reply.from(zr))
        } finally {
            zr.close()
        }
    }

internal fun helloCallbackOf(f: (Hello) -> Unit): (io.zenoh.jni.scouting.ZHello) -> Unit =
    { zh ->
        try {
            f(
                Hello(
                    WhatAmI.fromJni(io.zenoh.jni.scouting.zHelloWhatami(zh)),
                    ZenohId(io.zenoh.jni.scouting.zHelloZid(zh)),
                    io.zenoh.jni.scouting.zHelloLocators(zh)
                )
            )
        } finally {
            zh.close()
        }
    }
