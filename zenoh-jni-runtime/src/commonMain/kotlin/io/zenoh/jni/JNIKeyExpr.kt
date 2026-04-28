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
 * Alias for the auto-generated [KeyExpr] data class.
 *
 * `ptr == 0L` means "not declared on a session — use [KeyExpr.string] at the
 * native side"; `ptr != 0L` means "declared — the pointer holds the
 * `Arc<KeyExpr>` registration handle".
 */
typealias JNIKeyExpr = KeyExpr

fun KeyExpr.Companion.undeclared(keyExpr: String): KeyExpr = KeyExpr(ptr = 0L, string = keyExpr)

fun KeyExpr.Companion.of(declared: KeyExpr?, keyExpr: String): KeyExpr =
    declared ?: undeclared(keyExpr)

@Throws(ZError::class)
fun KeyExpr.Companion.tryFrom(keyExpr: String): KeyExpr = JNIKeyExprNative.tryFromViaJNI(keyExpr)

@Throws(ZError::class)
fun KeyExpr.Companion.autocanonize(keyExpr: String): KeyExpr =
    JNIKeyExprNative.autocanonizeViaJNI(keyExpr)

@Throws(ZError::class)
fun KeyExpr.Companion.intersects(a: KeyExpr?, aStr: String, b: KeyExpr?, bStr: String): Boolean =
    JNIKeyExprNative.intersectsViaJNI(of(a, aStr), of(b, bStr))

@Throws(ZError::class)
fun KeyExpr.Companion.includes(a: KeyExpr?, aStr: String, b: KeyExpr?, bStr: String): Boolean =
    JNIKeyExprNative.includesViaJNI(of(a, aStr), of(b, bStr))

@Throws(ZError::class)
fun KeyExpr.Companion.relationTo(a: KeyExpr?, aStr: String, b: KeyExpr?, bStr: String): Int =
    JNIKeyExprNative.relationToViaJNI(of(a, aStr), of(b, bStr))

@Throws(ZError::class)
fun KeyExpr.Companion.join(a: KeyExpr?, aStr: String, other: String): KeyExpr =
    JNIKeyExprNative.joinViaJNI(of(a, aStr), other)

@Throws(ZError::class)
fun KeyExpr.Companion.concat(a: KeyExpr?, aStr: String, other: String): KeyExpr =
    JNIKeyExprNative.concatViaJNI(of(a, aStr), other)

fun KeyExpr.close() = JNIKeyExprNative.dropKeyExprViaJNI(this)
