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

import io.zenoh.exceptions.ZError

/**
 * Helpers for the key-expression JNI surface.
 *
 * Operations that have a `#[prebindgen]` counterpart in `zenoh-flat`
 * (`try_from`, `autocanonize`, `intersects`, `includes`, `relation_to`,
 * `join`, `concat`) delegate to the generator-emitted wrappers in
 * [JNIWrappers]. The hand-written `dropKeyExprViaJNI` is retained as a
 * dedicated free entry point — it doesn't appear as a `#[prebindgen]`
 * fn because the consume semantic for a session-undeclare goes through
 * `undeclare_key_expr` instead.
 */

/**
 * Pick the declared handle if present, else the raw string. Returns
 * a JVM `Object` (boxed `java.lang.Long` or `java.lang.String`) which
 * the native dispatching converter resolves at runtime.
 */
fun keyExprArg(handle: Long?, str: String): Any = handle ?: str

@Throws(ZError::class)
fun keyExprTryFrom(keyExpr: String): String = JNIWrappers.tryFrom(keyExpr)

@Throws(ZError::class)
fun keyExprAutocanonize(keyExpr: String): String = JNIWrappers.autocanonize(keyExpr)

@Throws(ZError::class)
fun keyExprIntersects(a: Long?, aStr: String, b: Long?, bStr: String): Boolean =
    JNIWrappers.intersects(keyExprArg(a, aStr), keyExprArg(b, bStr))

@Throws(ZError::class)
fun keyExprIncludes(a: Long?, aStr: String, b: Long?, bStr: String): Boolean =
    JNIWrappers.includes(keyExprArg(a, aStr), keyExprArg(b, bStr))

@Throws(ZError::class)
fun keyExprRelationTo(a: Long?, aStr: String, b: Long?, bStr: String): Int =
    JNIWrappers.relationTo(keyExprArg(a, aStr), keyExprArg(b, bStr))

/** Result of a join/concat: `(handle, canonicalString)`. */
data class KeyExprResult(val handle: Long, val string: String)

@Throws(ZError::class)
fun keyExprJoin(a: Long?, aStr: String, other: String): KeyExprResult {
    val handle = JNIWrappers.join(keyExprArg(a, aStr), other).peek()
    // Match Rust's `KeyExpr::join` formatting: "{self}/{other}".
    return KeyExprResult(handle, "$aStr/$other")
}

@Throws(ZError::class)
fun keyExprConcat(a: Long?, aStr: String, other: String): KeyExprResult {
    val handle = JNIWrappers.concat(keyExprArg(a, aStr), other).peek()
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
