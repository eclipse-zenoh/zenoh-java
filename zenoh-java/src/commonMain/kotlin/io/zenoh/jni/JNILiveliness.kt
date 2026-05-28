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

package io.zenoh.jni

import io.zenoh.exceptions.ZError
import io.zenoh.handlers.Callback
import io.zenoh.jni.session.ZSession
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.liveliness.LivelinessToken
import io.zenoh.pubsub.CallbackSubscriber
import io.zenoh.pubsub.HandlerSubscriber
import io.zenoh.query.Reply
import io.zenoh.sample.Sample
import java.time.Duration

internal object JNILiveliness {

    @Throws(ZError::class)
    fun <R> get(
        jniSession: JNISession,
        keyExpr: KeyExpr,
        callback: Callback<Reply>,
        receiver: R,
        timeout: Duration,
        onClose: Runnable
    ): R {
        ZSession(jniSession.sessionPtr).livelinessGet(
            keyExpr.flat,
            timeout.toMillis(),
            io.zenoh.jni.callbacks.ReplyCallback { flat -> callback.run(Reply.from(flat)) },
            io.zenoh.jni.callbacks.Callback { onClose.run() }
        )
        return receiver
    }

    @Throws(ZError::class)
    fun declareToken(jniSession: JNISession, keyExpr: KeyExpr): LivelinessToken {
        val token = ZSession(jniSession.sessionPtr).zLivelinessDeclareToken(keyExpr.flat)
        return LivelinessToken(token)
    }

    @Throws(ZError::class)
    fun declareSubscriber(
        jniSession: JNISession,
        keyExpr: KeyExpr,
        callback: Callback<Sample>,
        history: Boolean,
        onClose: () -> Unit
    ): CallbackSubscriber {
        val zSubscriber = ZSession(jniSession.sessionPtr).livelinessDeclareSubscriber(
            keyExpr.flat,
            history,
            io.zenoh.jni.callbacks.SampleCallback { flat -> callback.run(Sample.from(flat)) },
            io.zenoh.jni.callbacks.Callback { onClose() }
        )
        return CallbackSubscriber(keyExpr, zSubscriber)
    }

    @Throws(ZError::class)
    fun <R> declareSubscriber(
        jniSession: JNISession,
        keyExpr: KeyExpr,
        callback: Callback<Sample>,
        receiver: R,
        history: Boolean,
        onClose: () -> Unit
    ): HandlerSubscriber<R> {
        val zSubscriber = ZSession(jniSession.sessionPtr).livelinessDeclareSubscriber(
            keyExpr.flat,
            history,
            io.zenoh.jni.callbacks.SampleCallback { flat -> callback.run(Sample.from(flat)) },
            io.zenoh.jni.callbacks.Callback { onClose() }
        )
        return HandlerSubscriber(keyExpr, zSubscriber, receiver)
    }
}
