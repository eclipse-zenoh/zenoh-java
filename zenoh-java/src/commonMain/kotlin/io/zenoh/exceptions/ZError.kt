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

package io.zenoh.exceptions

/**
 * Public Zenoh error surfaced from the high-level zenoh-java API.
 *
 * Independent of the JNI layer's own [io.zenoh.jni.ZError] — the two are
 * intentionally separate so the JNI bridge can keep its internal
 * `io.zenoh.jni.*` namespace self-contained and the public API can keep
 * its `io.zenoh.exceptions.*` namespace stable. The bridging happens at
 * the public boundary via [jniCall], which catches the JNI's exception
 * and rethrows it as this class with the same message.
 */
public class ZError(override val message: String? = null) : Exception()

/**
 * Run [block] (typically a single JNI delegation) and surface any
 * [io.zenoh.jni.ZError] it throws as the public [ZError] with the same
 * message. Every public `@Throws(ZError::class)` function in zenoh-java
 * routes through this helper so the declared exception type matches
 * the actual one — the JNI's internal exception class never leaks past
 * the API boundary.
 */
internal inline fun <T> jniCall(block: () -> T): T = try {
    block()
} catch (e: io.zenoh.jni.ZError) {
    throw ZError(e.message)
}
