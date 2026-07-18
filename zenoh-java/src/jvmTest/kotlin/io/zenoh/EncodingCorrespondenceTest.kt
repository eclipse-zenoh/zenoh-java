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

package io.zenoh

import io.zenoh.bytes.Encoding
import io.zenoh.exceptions.throwZError0
import io.zenoh.jni.bytes.Encoding as JniEncoding
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Correspondence tests for the pure-JVM [Encoding] implementation.
 *
 * The SDK implements the encoding string ↔ `(id, schema)` conversion in pure
 * Kotlin (no JNI crossing), on the contract that any JVM-side reimplementation
 * of zenoh-flat API must be verified against the native implementation. These
 * tests drive both implementations — the pure one and the native one (the
 * generated `Encoding` handle methods) — over the whole predefined id range
 * plus the edge shapes of the parse/render rules, asserting equal results.
 */
class EncodingCorrespondenceTest {

    /** Native implementation: the canonical string of `(id, schema)`. */
    private fun nativeRender(id: Int, schema: String?): String {
        val h = JniEncoding.newFromId(id, schema, throwZError0)
        try {
            return h.toStr(throwZError0)
        } finally {
            h.close()
        }
    }

    /** Native implementation: parse a string into `(id, schema, canonical string)`. */
    private fun nativeParse(s: String): Triple<Int, String?, String> {
        val h = JniEncoding.newFromString(s, throwZError0)
        try {
            return Triple(h.getId(throwZError0), h.getSchema(throwZError0), h.toStr(throwZError0))
        } finally {
            h.close()
        }
    }

    @Test
    fun renderMatchesNativeAcrossIdRange() {
        // The whole predefined range, a gap past it (unknown ids), and the
        // custom id — with and without a schema.
        val ids = (0..64) + listOf(1000, 0xFFFE, 0xFFFF)
        for (id in ids) {
            for (schema in listOf(null, "utf-8")) {
                assertEquals(
                    "render mismatch for (id=$id, schema=$schema)",
                    nativeRender(id, schema),
                    Encoding(id, schema).toString(),
                )
            }
        }
    }

    @Test
    fun parseMatchesNativeOnNamesAndEdges() {
        // Every predefined canonical name (obtained FROM the native table so the
        // corpus can't drift), plus the parse-rule edge shapes.
        val names = (0..52).map { nativeRender(it, null) }
        val edges = listOf(
            "",
            "text/plain",
            "text/plain;utf-8",
            "text/plain;",
            "my_custom_encoding",
            "custom;with;semicolons",
            ";leading_separator",
            "unknown_name;schema",
            "zenoh/bytes;s",
        )
        for (s in names + names.map { "$it;schema" } + edges) {
            val (nid, nschema, nstr) = nativeParse(s)
            val pure = Encoding.from(s)
            assertEquals("id mismatch for \"$s\"", nid, pure.id)
            assertEquals("schema mismatch for \"$s\"", nschema, pure.schema)
            assertEquals("render mismatch for \"$s\"", nstr, pure.toString())
        }
    }

    @Test
    fun withSchemaMatchesNative() {
        val bases = listOf(
            Encoding.TEXT_PLAIN,
            Encoding.ZENOH_BYTES,
            Encoding.from("my_custom_encoding"),
            Encoding.from("text/plain;old-schema"),
        )
        for (base in bases) {
            val pure = base.withSchema("new-schema")
            // The `e` param crosses on the selector's value arm as its
            // decomposed (id, schema) pair.
            val w = JniEncoding.newWithSchema(0, base.id, base.schema, null, "new-schema", throwZError0)
            val nativeStr = try {
                w.toStr(throwZError0)
            } finally {
                w.close()
            }
            assertEquals("withSchema mismatch for $base", nativeStr, pure.toString())
        }
    }
}
