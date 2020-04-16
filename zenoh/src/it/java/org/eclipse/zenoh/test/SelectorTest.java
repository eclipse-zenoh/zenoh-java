/*
 * Copyright (c) 2017, 2020 ADLINK Technology Inc.
 *
 * This program and the accompanying materials are made available under the
 * terms of the Eclipse Public License 2.0 which is available at
 * http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
 * which is available at https://www.apache.org/licenses/LICENSE-2.0.
 *
 * SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
 *
 * Contributors:
 *   ADLINK zenoh team, <zenoh@adlink-labs.tech>
 */
package org.eclipse.zenoh.test;

import org.junit.Assert;
import org.junit.Test;

import org.eclipse.zenoh.*;

public class SelectorTest {

    private void testSelector(String s, String expPath, String expFilter, String expFragment) {
        try {
            Selector sel = new Selector(s);
            Assert.assertEquals("Selector for " + s + " has unexpected path: " + sel.getPath(), expPath, sel.getPath());
            Assert.assertEquals("Selector for " + s + " has unexpected predicate: " + sel.getFilter(), expFilter,
                    sel.getFilter());
            Assert.assertEquals("Selector for " + s + " has unexpected fragment: " + sel.getFragment(), expFragment,
                    sel.getFragment());
        } catch (Throwable e) {
            e.printStackTrace();
            Assert.fail(e.toString());
        }

    }

    @Test
    public final void testSelectors() {
        testSelector("/a/b/c", "/a/b/c", "", "");
        testSelector("/a/b/c?xyz", "/a/b/c", "xyz", "");
        testSelector("/a/b/c#xyz", "/a/b/c", "", "xyz");
        testSelector("/a/b/c?ghi?xyz", "/a/b/c", "ghi?xyz", "");
        testSelector("/a/b/c#ghi#xyz", "/a/b/c", "", "ghi#xyz");
        testSelector("/a/b/c?ghi#xyz", "/a/b/c", "ghi", "xyz");
        testSelector("/a/b/c#ghi?xyz", "/a/b/c", "", "ghi?xyz");
        testSelector("/*/b/**", "/*/b/**", "", "");
    }

}
