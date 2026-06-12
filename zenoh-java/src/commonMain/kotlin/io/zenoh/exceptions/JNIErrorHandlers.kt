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
import io.zenoh.jni.errors.ZErrorHandler

/**
 * Error-callback handlers passed to the generated flat-jni wrappers. In the
 * canonical model a wrapper never throws from native code — on failure it
 * invokes its trailing `onError` handler (a generated typed `fun interface`).
 * These handlers throw [ZError] directly, so the SDK no longer needs a
 * `try/catch` translation layer.
 *
 * `je` is the binding-layer error (UTF-8 decode, closed handle, …); `ze` is the
 * library (zenoh) error message. Exactly one is set. The handlers bind the
 * interface's `out R` to [Nothing], which is a subtype of every `R`, so a
 * single instance satisfies every wrapper's `onError` regardless of its
 * return type (declaration-site covariance).
 */

/** Handler for a fallible wrapper (`Result<_, ZError>`): `run(je, message)`. */
internal val throwZError: ZErrorHandler<Nothing> =
    ZErrorHandler { je, message -> throw ZError(je ?: message ?: "unknown native error") }

/** Handler for an infallible wrapper (binding errors only): `run(je)`. */
internal val throwZError0: JniErrorHandler<Nothing> =
    JniErrorHandler { je -> throw ZError(je ?: "native binding error") }
