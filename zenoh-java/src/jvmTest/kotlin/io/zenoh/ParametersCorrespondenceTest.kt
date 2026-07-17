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

package io.zenoh

import io.zenoh.exceptions.throwZError0
import io.zenoh.jni.query.Parameters as JniParameters
import io.zenoh.jni.query.parametersGet
import io.zenoh.jni.query.parametersInsert
import io.zenoh.jni.query.parametersIsWellFormed
import io.zenoh.jni.query.parametersRemove
import org.junit.Assert.assertEquals
import org.junit.Test
import kotlin.random.Random

/**
 * Correspondence tests for the shared pure-JVM
 * [io.zenoh.jni.query.Parameters] implementation.
 *
 * The shared tier implements selector-parameters parsing in pure Kotlin (no
 * JNI crossing on the production path), on the contract that any JVM-side
 * reimplementation of zenoh-flat semantics must be verified against the
 * native implementation. These tests drive both — the pure implementation and
 * the native oracle (`parametersGet`/`parametersInsert`/`parametersRemove`/
 * `parametersIsWellFormed`, thin wrappers over `zenoh::query::Parameters`) —
 * over edge shapes and randomized inputs, asserting equal results.
 */
class ParametersCorrespondenceTest {

    /** Edge shapes of the format rules. */
    private val edgeCases = listOf(
        "",
        ";",
        ";;",
        "a",
        "a=",
        "=v",
        "=",
        "a=1",
        "a=1;b=2",
        "a=1;a=2",
        "a=b=c",
        "a==",
        "k=%zz",
        "k=%20",
        "flag;a=1",
        ";;a=1;;b=2;;",
        "c=1|2|3",
        "c=|",
        "c=ified|",
        "ключ=значение",
        "a=1;=2;b=3",
        " a = 1 ; b = 2 ",
        "a;a;a",
        "a=1;b;a=2",
    )

    private val keys = listOf("a", "b", "c", "k", "flag", "", "missing", "ключ", " a ")

    private fun assertCorrespondence(s: String) {
        val pure = JniParameters.fromString(s)
        for (k in keys) {
            assertEquals(
                "get(\"$k\") diverges for input \"$s\"",
                parametersGet(s, k, throwZError0),
                pure.get(k),
            )
        }
        assertEquals(
            "isWellFormed diverges for input \"$s\"",
            parametersIsWellFormed(s, throwZError0),
            pure.isWellFormed(),
        )
        for (k in keys) {
            assertEquals(
                "insert(\"$k\", \"val\") diverges for input \"$s\"",
                parametersInsert(s, k, "val", throwZError0),
                JniParameters.fromString(s).also { it.insert(k, "val") }.asString(),
            )
        }
        // `remove` is deliberately NOT oracle-compared: see
        // [nativeRemoveBugCanary].
    }

    @Test
    fun edgeCasesMatchNativeOracle() {
        for (s in edgeCases) {
            assertCorrespondence(s)
        }
    }

    @Test
    fun randomizedInputsMatchNativeOracle() {
        // Random strings over an alphabet dense in separators, so structural
        // collisions (duplicate keys, empty chunks, '=' in values) are common.
        val alphabet = "ab;=|%; =;"
        val rng = Random(20260718)
        repeat(500) {
            val s = buildString {
                repeat(rng.nextInt(0, 24)) { append(alphabet[rng.nextInt(alphabet.length)]) }
            }
            assertCorrespondence(s)
        }
    }

    /** The shared implementation follows Rust's DOCUMENTED remove contract
     * ("preserving the insertion order"). */
    @Test
    fun removeFollowsTheDocumentedContract() {
        fun removed(s: String, k: String): String =
            JniParameters.fromString(s).also { it.remove(k) }.asString()

        assertEquals("b=2;c=3", removed("b=2;a=1;c=3", "a"))
        assertEquals("b=2", removed("a=1;b=2;a=3", "a"))
        assertEquals("x=1;y=2", removed("x=1;y=2", "missing"))
        assertEquals("", removed("a=1", "a"))
        assertEquals("a=1", removed("flag;a=1", "flag"))
        assertEquals("1", JniParameters.fromString("b=2;a=1").remove("a"))
    }

    /** CANARY: upstream zenoh's `parameters::remove` has an
     * iterator-consumption bug — `find` advances past every entry up to and
     * including the first match before `filter` builds the result, so
     * entries PRECEDING the match are dropped, and removing an absent key
     * erases the whole string. The shared implementation intentionally
     * diverges (see [removeFollowsTheDocumentedContract]). This test pins
     * the buggy native behavior: when upstream fixes it, this test fails —
     * then delete it and fold `remove` into [assertCorrespondence]. */
    @Test
    fun nativeRemoveBugCanary() {
        assertEquals("c=3", parametersRemove("b=2;a=1;c=3", "a", throwZError0))
        assertEquals("", parametersRemove("x=1;y=2", "missing", throwZError0))
    }
}
