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

package io.zenoh.exceptions

import io.zenoh.jni.JniErrorHandler
import io.zenoh.jni.errors.ErrorHandler

/**
 * Error-callback handlers passed to the generated flat-jni wrappers. In the
 * canonical model a wrapper never throws from native code — on failure it
 * invokes an error handler (a generated typed `fun interface`). These handlers
 * throw [ZError] directly, so the SDK no longer needs a `try/catch` layer.
 *
 * The generated protocol has two independent channels (prebindgen #45): a
 * fallible wrapper takes `onBindingError` (a [JniErrorHandler], any
 * binding-layer failure — UTF-8 decode, closed handle, …) followed by
 * `onError` (the typed domain [ErrorHandler], the decomposed zenoh error);
 * an infallible wrapper takes only the binding [JniErrorHandler]. The handlers
 * bind the interface's `out R` to [Nothing], a subtype of every `R`, so a
 * single instance satisfies every wrapper regardless of its return type
 * (declaration-site covariance).
 */

/** Domain handler for a fallible wrapper (`Result<_, ZError>`): `run(message)`. */
internal val throwZError: ErrorHandler<Nothing> =
    ErrorHandler { message -> throw ZError(message) }

/** Binding handler — the `onBindingError` of a fallible wrapper AND the sole
 * `onError` of an infallible one: `run(je)`. */
internal val throwZError0: JniErrorHandler<Nothing> =
    JniErrorHandler { je -> throw ZError(je ?: "native binding error") }
