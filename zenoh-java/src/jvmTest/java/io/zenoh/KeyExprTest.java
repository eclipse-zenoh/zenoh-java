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

package io.zenoh;

import io.zenoh.exceptions.ZError;
import io.zenoh.keyexpr.KeyExpr;
import io.zenoh.keyexpr.SetIntersectionLevel;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.junit.runners.JUnit4;

import static org.junit.Assert.*;

@RunWith(JUnit4.class)
public class KeyExprTest {

    @Test
    public void creation_TryFromTest() throws ZError {
        // A couple of examples of valid and invalid key expressions.
        KeyExpr keyExpr = KeyExpr.tryFrom("example/test");

        assertThrows(ZError.class, () -> KeyExpr.tryFrom("example/test?param='test'"));

        KeyExpr keyExpr3 = KeyExpr.tryFrom("example/*/test");

        assertThrows(ZError.class, () -> KeyExpr.tryFrom("example/!*/test"));
    }

    @Test
    public void equalizationTest() throws ZError {
        KeyExpr keyExpr1 = KeyExpr.tryFrom("example/test");
        KeyExpr keyExpr2 = KeyExpr.tryFrom("example/test");
        assertEquals(keyExpr1, keyExpr2);

        KeyExpr keyExpr3 = KeyExpr.tryFrom("different/key/expr");
        assertNotEquals(keyExpr1, keyExpr3);
    }

    @Test
    public void creation_autocanonizeTest() throws ZError {
        KeyExpr keyExpr1 = KeyExpr.autocanonize("example/**/test");
        KeyExpr keyExpr2 = KeyExpr.autocanonize("example/**/**/test");
        assertEquals(keyExpr1, keyExpr2);
    }

    @Test
    public void toStringTest() throws ZError {
        String keyExprStr = "example/test/a/b/c";
        KeyExpr keyExpr = KeyExpr.tryFrom(keyExprStr);
        assertEquals(keyExprStr, keyExpr.toString());
        assertEquals(keyExprStr, keyExpr.toString());
    }

    @Test
    public void intersectionTest() throws ZError {
        KeyExpr keyExprA = KeyExpr.tryFrom("example/*/test");

        KeyExpr keyExprB = KeyExpr.tryFrom("example/B/test");
        assertTrue(keyExprA.intersects(keyExprB));

        KeyExpr keyExprC = KeyExpr.tryFrom("example/B/C/test");
        assertFalse(keyExprA.intersects(keyExprC));

        KeyExpr keyExprA2 = KeyExpr.tryFrom("example/**");
        assertTrue(keyExprA2.intersects(keyExprC));
    }

    @Test
    public void includesTest() throws ZError {
        KeyExpr keyExpr = KeyExpr.tryFrom("example/**");
        KeyExpr includedKeyExpr = KeyExpr.tryFrom("example/A/B/C/D");
        assertTrue(keyExpr.includes(includedKeyExpr));

        KeyExpr notIncludedKeyExpr = KeyExpr.tryFrom("C/D");
        assertFalse(keyExpr.includes(notIncludedKeyExpr));
    }

    @Test
    public void sessionDeclarationTest() throws ZError {
        Session session = Zenoh.open(Config.loadDefault());
        KeyExpr keyExpr = session.declareKeyExpr("a/b/c");
        assertEquals("a/b/c", keyExpr.toString());
        session.close();
        keyExpr.close();
    }

    @Test
    public void sessionUnDeclarationTest() throws ZError {
        Session session = Zenoh.open(Config.loadDefault());
        KeyExpr keyExpr = session.declareKeyExpr("a/b/c");
        assertEquals("a/b/c", keyExpr.toString());

        session.undeclare(keyExpr);

        // Undeclaring twice a key expression shall fail.
        assertThrows(ZError.class, () -> session.undeclare(keyExpr));

        // Undeclaring a key expr that was not declared through a session.
        KeyExpr keyExpr2 = KeyExpr.tryFrom("x/y/z");
        assertThrows(ZError.class, () -> session.undeclare(keyExpr2));

        session.close();
    }

    @Test
    public void relationTo_includesTest() throws ZError {
        KeyExpr keyExprA = KeyExpr.tryFrom("A/**");
        KeyExpr keyExprB = KeyExpr.tryFrom("A/B/C");

        assertEquals(SetIntersectionLevel.INCLUDES, keyExprA.relationTo(keyExprB));
    }

    @Test
    public void relationTo_intersectsTest() throws ZError {
        KeyExpr keyExprA = KeyExpr.tryFrom("A/*/C/D");
        KeyExpr keyExprB = KeyExpr.tryFrom("A/B/C/*");

        assertEquals(SetIntersectionLevel.INTERSECTS, keyExprA.relationTo(keyExprB));
    }

    @Test
    public void relationTo_equalsTest() throws ZError {
        KeyExpr keyExprA = KeyExpr.tryFrom("A/B/C");
        KeyExpr keyExprB = KeyExpr.tryFrom("A/B/C");

        assertEquals(SetIntersectionLevel.EQUALS, keyExprA.relationTo(keyExprB));
    }

    @Test
    public void relationTo_disjointTest() throws ZError {
        KeyExpr keyExprA = KeyExpr.tryFrom("A/B/C");
        KeyExpr keyExprB = KeyExpr.tryFrom("D/E/F");

        assertEquals(SetIntersectionLevel.DISJOINT, keyExprA.relationTo(keyExprB));
    }

    @Test
    public void joinTest() throws ZError {
        KeyExpr keyExprA = KeyExpr.tryFrom("A/B");
        KeyExpr keyExprExpected = KeyExpr.tryFrom("A/B/C/D");

        KeyExpr keyExprJoined = keyExprA.join("C/D");
        assertEquals(keyExprExpected, keyExprJoined);
    }

    @Test
    public void concatTest() throws ZError {
        KeyExpr keyExprA = KeyExpr.tryFrom("A/B");
        KeyExpr keyExprExpected = KeyExpr.tryFrom("A/B/C/D");

        KeyExpr keyExprConcat = keyExprA.concat("/C/D");
        assertEquals(keyExprExpected, keyExprConcat);
    }
}
