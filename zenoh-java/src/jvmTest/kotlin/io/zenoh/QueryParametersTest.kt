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

import io.zenoh.exceptions.throwZError
import io.zenoh.exceptions.throwZError0
import io.zenoh.keyexpr.KeyExpr
import io.zenoh.query.Parameters
import io.zenoh.query.Query
import io.zenoh.query.Reply
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The receive path must be as safe against malicious external data as the
 * Rust layer: there a selector's parameters are an unvalidated string view —
 * any string is valid and forwarded untouched — so [Parameters] accepts
 * anything, with Rust semantics.
 */
class QueryParametersTest {

    @Test
    fun parsingAcceptsAnyRemoteInputWithRustSemantics() {
        // Duplicated parameter name: accepted, the FIRST occurrence wins on get.
        assertEquals("1", Parameters.from("a=1;a=2").get("a"))

        // No percent-decoding: the value is kept verbatim.
        assertEquals("%zz", Parameters.from("k=%zz").get("k"))
        assertEquals("%20", Parameters.from("k=%20").get("k"))

        // Value containing '=': split on the FIRST '=' only.
        assertEquals("b=c", Parameters.from("a=b=c").get("a"))

        // Flag without a value, empty chunks, blank input: all accepted.
        assertEquals("", Parameters.from("flag").get("flag"))
        assertEquals("1", Parameters.from(";;a=1;;").get("a"))
        assertTrue(Parameters.from("").isEmpty())

        // The string round-trips verbatim (duplicates preserved) until an
        // insert/remove normalizes it.
        val p = Parameters.from("a=1;a=2;flag")
        assertEquals("a=1;a=2;flag", p.toString())
        p.insert("a", "3")
        assertEquals("flag;a=3", p.toString())
    }

    @Test
    fun queryableReceivesMalformedParametersQuery() {
        // A remote (non-JVM) client is free to send selector parameters that
        // a URL-style parser would reject — the Rust layer forwards any
        // string untouched. The queryable must still receive the query and
        // be able to reply.
        val session = Zenoh.open(Config.loadDefault())
        val keyExpr = KeyExpr.tryFrom("example/testing/malformed/params")
        var received: Query? = null
        val queryable = session.declareQueryable(keyExpr, callback = { query ->
            received = query
            query.reply(keyExpr, "ok")
        })

        var reply: Reply? = null
        // Bypass the JVM-side Selector validation, as a remote client would.
        session.zSession!!.get(
            keyExpr.toString(),             // s
            "a=1;a=2;bad=%zz",              // parameters
            1000L,                          // timeoutMs
            null,                           // target
            null,                           // consolidation
            null,                           // acceptReplies
            null,                           // congestionControl
            null,                           // priority
            null,                           // express
            null,                           // payload
            -1,                             // encodingSel (absent)
            null,                           // encoding00
            null,                           // encoding01
            null,                           // encoding1
            null,                           // attachment
            replyCallbackOf { reply = it },
            {},                             // onClose
            throwZError0, throwZError,
        )
        Thread.sleep(1000)

        assertNotNull(received)
        assertEquals("1", received!!.parameters?.get("a"))
        assertEquals("%zz", received!!.parameters?.get("bad"))
        assertNotNull(reply)

        queryable.close()
        session.close()
    }
}
