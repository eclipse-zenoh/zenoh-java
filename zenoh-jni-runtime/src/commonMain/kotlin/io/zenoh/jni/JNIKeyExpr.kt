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

/**
 * Helpers for the key-expression JNI surface.
 *
 * The native side accepts `Any` for every key-expr parameter and
 * dispatches at runtime: a [Long] is treated as the
 * `Arc<KeyExpr<'static>>` registration handle, anything else
 * (a [String] in practice) is validated and converted to an owned
 * `KeyExpr<'static>`. Pass `handle ?: keyExprString` to native fns —
 * the `?:` operator picks the boxed [Long] when present, otherwise the
 * raw string.
 *
 * Functions that *return* a key-expression hand back a [Long] (the
 * Arc pointer); the canonical string is computed locally on the
 * Kotlin side. For [join] / [concat] the result string matches
 * Rust's `format!("{a}/{other}")` / `format!("{a}{other}")` since
 * the native validation is non-rewriting.
 */

/**
 * Pick the declared handle if present, else the raw string. Returns
 * a JVM `Object` (boxed `java.lang.Long` or `java.lang.String`) which
 * the native dispatching converter resolves at runtime.
 */
fun keyExprArg(handle: Long?, str: String): Any = handle ?: str

@Throws(ZError::class)
fun keyExprTryFrom(keyExpr: String): String = JNINative.tryFromViaJNI(keyExpr)

@Throws(ZError::class)
fun keyExprAutocanonize(keyExpr: String): String = JNINative.autocanonizeViaJNI(keyExpr)

@Throws(ZError::class)
fun keyExprIntersects(a: Long?, aStr: String, b: Long?, bStr: String): Boolean =
    JNINative.intersectsViaJNI(keyExprArg(a, aStr), keyExprArg(b, bStr))

@Throws(ZError::class)
fun keyExprIncludes(a: Long?, aStr: String, b: Long?, bStr: String): Boolean =
    JNINative.includesViaJNI(keyExprArg(a, aStr), keyExprArg(b, bStr))

@Throws(ZError::class)
fun keyExprRelationTo(a: Long?, aStr: String, b: Long?, bStr: String): Int =
    JNINative.relationToViaJNI(keyExprArg(a, aStr), keyExprArg(b, bStr))

/** Result of a join/concat: `(handle, canonicalString)`. */
data class KeyExprResult(val handle: Long, val string: String)

@Throws(ZError::class)
fun keyExprJoin(a: Long?, aStr: String, other: String): KeyExprResult {
    val handle = JNINative.joinViaJNI(keyExprArg(a, aStr), other)
    // Match Rust's `KeyExpr::join` formatting: "{self}/{other}".
    return KeyExprResult(handle, "$aStr/$other")
}

@Throws(ZError::class)
fun keyExprConcat(a: Long?, aStr: String, other: String): KeyExprResult {
    val handle = JNINative.concatViaJNI(keyExprArg(a, aStr), other)
    // Match Rust's `KeyExpr::concat` formatting: "{self}{other}".
    return KeyExprResult(handle, "$aStr$other")
}

/**
 * Release the native `Arc<KeyExpr>` registration. No-op for `0L`
 * (string-only keyexprs never allocated an Arc).
 */
fun keyExprDrop(handle: Long) {
    if (handle != 0L) JNINative.dropKeyExprViaJNI(handle)
}
