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

@file:JvmName("JNIBytes")

package io.zenoh.jni.bytes

import io.zenoh.ZenohLoad
import java.lang.reflect.Type

// Forces the native library to load before the first JNI call. The standalone
// (de)serialization path has no Session to trigger the load, and the facade
// class <clinit> runs before either external fn below is invoked.
private val ensureLoaded = ZenohLoad

// `onError` mirrors the generated wrappers' error callback: on a serialization
// failure the native side invokes it with the message (the binding-error `je`
// arity), and the handler throws — no direct throw from native code.
internal external fun serializeViaJNI(any: Any, type: Type, onError: io.zenoh.jni.JniErrorHandler<ByteArray>): ByteArray

internal external fun deserializeViaJNI(bytes: ByteArray, type: Type, onError: io.zenoh.jni.JniErrorHandler<Any>): Any
