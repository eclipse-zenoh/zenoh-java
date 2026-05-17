//
// Copyright (c) 2026 ZettaScale Technology
//
// This program and the accompanying materials are made available under the
// terms of the Eclipse Public License 2.0 which is available at
// http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
// which is available at https://www.apache.org/legal/epl-2.0.
//
// SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
//
// Contributors:
//   ZettaScale Zenoh Team, <zenoh@zettascale.tech>
//

package io.zenoh.jni

import io.zenoh.ZenohLoad
import io.zenoh.exceptions.ZError
import io.zenoh.jni.callbacks.JNIOnCloseCallback
import io.zenoh.jni.callbacks.JNIScoutCallback

/** Typed [NativeHandle] for a native Zenoh `Scout`. */
public class JNIScout(initialPtr: Long) : NativeHandle(initialPtr) {

    companion object {
        init {
            ZenohLoad
        }

        @Throws(ZError::class)
        fun scout(
            whatAmI: Int,
            callback: JNIScoutCallback,
            onClose: JNIOnCloseCallback,
            config: JNIConfig?,
        ): JNIScout = JNIScout(scoutViaJNI(whatAmI, callback, onClose, config?.peek() ?: 0))

        @Throws(ZError::class)
        private external fun scoutViaJNI(
            whatAmI: Int,
            callback: JNIScoutCallback,
            onClose: JNIOnCloseCallback,
            configPtr: Long,
        ): Long

        private external fun freePtrViaJNI(ptr: Long)
    }

    fun close() = close { freePtrViaJNI(it) }
}
