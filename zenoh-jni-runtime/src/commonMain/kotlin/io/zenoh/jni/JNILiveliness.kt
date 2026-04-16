//
// Copyright (c) 2026 ZettaScale Technology
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
import io.zenoh.jni.callbacks.JNIGetCallback
import io.zenoh.jni.callbacks.JNIOnCloseCallback
import io.zenoh.jni.callbacks.JNISubscriberCallback

/** Adapter object for interacting with Zenoh Liveliness through JNI. */
public object JNILiveliness {

    @Throws(ZError::class)
    fun declareToken(sessionPtr: Long, keyExprPtr: Long, keyExprString: String): JNILivelinessToken =
        JNILivelinessToken(declareTokenViaJNI(sessionPtr, keyExprPtr, keyExprString))

    @Throws(ZError::class)
    private external fun declareTokenViaJNI(sessionPtr: Long, keyExprPtr: Long, keyExprString: String): Long

    @Throws(ZError::class)
    fun declareSubscriber(
        sessionPtr: Long,
        keyExprPtr: Long,
        keyExprString: String,
        callback: JNISubscriberCallback,
        history: Boolean,
        onClose: JNIOnCloseCallback,
    ): JNISubscriber = JNISubscriber(declareSubscriberViaJNI(sessionPtr, keyExprPtr, keyExprString, callback, history, onClose))

    @Throws(ZError::class)
    private external fun declareSubscriberViaJNI(
        sessionPtr: Long,
        keyExprPtr: Long,
        keyExprString: String,
        callback: JNISubscriberCallback,
        history: Boolean,
        onClose: JNIOnCloseCallback,
    ): Long

    @Throws(ZError::class)
    external fun getViaJNI(
        sessionPtr: Long,
        keyExprPtr: Long,
        keyExprString: String,
        callback: JNIGetCallback,
        timeoutMs: Long,
        onClose: JNIOnCloseCallback,
    )
}
