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

/** Adapter class for a native Zenoh LivelinessToken. */
public class JNILivelinessToken(private val ptr: Long) {

    fun undeclare() {
        undeclareViaJNI(ptr)
    }

    companion object {
        private external fun undeclareViaJNI(ptr: Long)
    }
}
