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

/**
 * Error-callback handlers passed to the generated flat-jni wrappers. In the
 * canonical model a wrapper never throws from native code — on failure it
 * invokes its trailing `onError` callback. These handlers throw [ZError]
 * directly, so the SDK no longer needs a `try/catch` translation layer.
 *
 * `je` is the binding-layer error (UTF-8 decode, closed handle, …); `ze` is the
 * library (zenoh) error message. Exactly one is set. Return type is [Nothing],
 * which is a subtype of every `R`, so a single handler satisfies every
 * `(String?, …) -> R` / `(String?) -> R` callback regardless of the wrapper's
 * return type (Kotlin function return-type covariance).
 */

/** Handler for a fallible wrapper (`Result<_, ZError>`): `(je, ze) -> R`. */
internal val throwZError: (String?, String) -> Nothing = { je, ze -> throw ZError(je ?: ze) }

/** Handler for an infallible wrapper (binding errors only): `(je) -> R`. */
internal val throwZError0: (String?) -> Nothing = { je -> throw ZError(je ?: "native binding error") }
