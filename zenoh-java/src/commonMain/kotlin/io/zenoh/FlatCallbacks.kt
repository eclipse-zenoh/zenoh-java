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
 * Adapters from the raw JNI callback fun-interfaces (each delivers a single
 * opaque native handle — `ZSample`/`ZQuery`/`ZReply`/`ZHello`) to a plain
 * `(SdkType) -> Unit`. The SDK object graph is reconstructed from the handle's
 * `z_*` accessors; the delivered handle is closed afterwards, except `ZQuery`,
 * which the resulting [Query] retains to reply through.
 */

internal fun sampleCallbackOf(f: (Sample) -> Unit): io.zenoh.jni.callbacks.ZSampleCallback =
    io.zenoh.jni.callbacks.ZSampleCallback { zs ->
        try {
            f(Sample.from(zs))
        } finally {
            zs.close()
        }
    }

internal fun queryCallbackOf(f: (Query) -> Unit): io.zenoh.jni.callbacks.ZQueryCallback =
    io.zenoh.jni.callbacks.ZQueryCallback { zq ->
        // The [Query] retains `zq` (its reply methods consume it), so it is NOT
        // closed here.
        f(Query.from(zq))
    }

internal fun replyCallbackOf(f: (Reply) -> Unit): io.zenoh.jni.callbacks.ZReplyCallback =
    io.zenoh.jni.callbacks.ZReplyCallback { zr ->
        try {
            f(Reply.from(zr))
        } finally {
            zr.close()
        }
    }

internal fun helloCallbackOf(f: (Hello) -> Unit): io.zenoh.jni.callbacks.ZHelloCallback =
    io.zenoh.jni.callbacks.ZHelloCallback { zh ->
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
