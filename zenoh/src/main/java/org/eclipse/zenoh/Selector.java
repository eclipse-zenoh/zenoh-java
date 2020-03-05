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
package org.eclipse.zenoh;

import java.util.regex.Pattern;
import java.util.regex.Matcher;

/**
 * A zenoh Selector is a string which is the conjunction of an path expression
 * identifying a set of keys and some optional parts allowing to refine the set
 * of {@link Path}s and associated {@link Value}s. Structure of a selector:
 * 
 * <pre>
 * /s1/s2/../sn?x>1&y<2&..&z=4(p1=v1;p2=v2;..;pn=vn)#a;x;y;..;z
 * |          | |            | |                  |  |        |
 * |-- expr --| |-- filter --| |--- properties ---|  |fragment|
 * </pre>
 * 
 * where:
 * <ul>
 * <li><b>expr</b>: is a path expression. I.e. a string similar to a
 * {@link Path} but with character `'*'` allowed. A single `'*'` matches any set
 * of characters in a path, except `'/'`. While `"**"` matches any set of
 * characters in a path, including `'/'`. A path expression can be absolute
 * (i.e. starting with a `'/'`) or relative to a {@link Workspace}.</li>
 *
 * <li><b>filter</b>: a list of predicates separated by `'&'` allowing to
 * perform filtering on the {@link Value} associated with the matching keys.
 * Each predicate has the form "`field``operator``value`" where:
 * <ul>
 * <li><b>field</b> is the name of a field in the value (is applicable and is
 * existing. otherwise the predicate is false)
 * <li><b>operator</b> is one of a comparison operators: `<` , `>` , `<=` , `>=`
 * , `=` , `!=`
 * <li><b>value</b> is the the value to compare the field's value with
 * </ul>
 * </li>
 *
 * <li><b>fragment</b>: a list of fields names allowing to return a sub-part of
 * each value. This feature only applies to structured values using a
 * "self-describing" encoding, such as JSON or XML. It allows to select only
 * some fields within the structure. A new structure with only the selected
 * fields will be used in place of the original value.</li>
 * </ul>
 * <p>
 * <b>NOTE: <i>the filters and fragments are not yet supported in current zenoh
 * version.</i></b>
 *
 * @see Workspace#get(Selector)
 * @see Workspace#subscribe(Selector, Listener)
 */
public final class Selector implements Comparable<Selector> {

    private static final String REGEX_PATH = "[^\\[\\]?#]+";
    private static final String REGEX_FILTER = "[^\\[\\]\\(\\)#]+";
    private static final String REGEX_PROPERTIES = ".*";
    private static final String REGEX_FRAGMENT = ".*";
    private static final Pattern PATTERN = Pattern.compile(String.format("(%s)(\\?(%s)?(\\((%s)\\))?)?(#(%s))?",
            REGEX_PATH, REGEX_FILTER, REGEX_PROPERTIES, REGEX_FRAGMENT));

    private String path;
    private String filter;
    private String properties;
    private String fragment;
    private String optionalPart;
    private String toString;

    /**
     * Create a Selector from a string such as "/demo/example/**?(name=Bob)".
     *
     * @param p the string
     */
    public Selector(String s) throws IllegalArgumentException {
        if (s == null) {
            throw new NullPointerException("The given selector is null");
        }
        if (s.isEmpty()) {
            throw new IllegalArgumentException("Invalid selector (empty String)");
        }

        Matcher m = PATTERN.matcher(s);
        if (!m.matches()) {
            throw new IllegalArgumentException("Invalid selector (not matching regex)");
        }
        String path = m.group(1);
        String filter = m.group(3);
        if (filter == null)
            filter = "";
        String properties = m.group(5);
        if (properties == null)
            properties = "";
        String fragment = m.group(7);
        if (fragment == null)
            fragment = "";

        init(path, filter, properties, fragment);
    }

    private Selector(String path, String filter, String properties, String fragment) {
        init(path, filter, properties, fragment);
    }

    private void init(String path, String filter, String properties, String fragment) {
        this.path = path;
        this.filter = filter;
        this.properties = properties;
        this.fragment = fragment;
        this.optionalPart = String.format("%s%s%s", filter, (properties.length() > 0 ? "(" + properties + ")" : ""),
                (fragment.length() > 0 ? "#" + fragment : ""));
        this.toString = path + (optionalPart.length() > 0 ? "?" + optionalPart : "");
    }

    /**
     * Return the path expression of this Selector. I.e. the substring before the
     * '?' character.
     * 
     * @return the path expression.
     */
    public String getPath() {
        return path;
    }

    /**
     * Return the filter expression of this Selector. I.e. the substring after the
     * '?' character and before the '(' character (or until end of string).
     * 
     * @return the filter expression or an empty string if not present.
     */
    public String getFilter() {
        return filter;
    }

    /**
     * Return the properties expression of this Selector. I.e. the substring
     * enclosed between '(' and ')' after the '?' character.
     * 
     * @return the properties expression or an empty string if not present.
     */
    public String getProperties() {
        return properties;
    }

    /**
     * Return the fragment expression of this Selector. I.e. the substring after the
     * '#' character.
     * 
     * @return the fragment expression or an empty string if not present.
     */
    public String getFragment() {
        return fragment;
    }

    protected String getOptionalPart() {
        return optionalPart;
    }

    @Override
    public String toString() {
        return toString;
    }

    @Override
    public int compareTo(Selector s) {
        return this.toString.compareTo(s.toString);
    }

    @Override
    public boolean equals(Object object) {
        if (object == null) {
            return false;
        }
        if (this == object) {
            return true;
        }
        if (object instanceof Selector) {
            return this.toString.equals(((Selector) object).toString);
        }
        return false;
    }

    @Override
    public int hashCode() {
        return toString.hashCode();
    }

    /**
     * Returns true if the Selector is relative (i.e. it doesn't start with '/')
     * 
     * @return true if the Selector is relative.
     */
    public boolean isRelative() {
        return path.length() == 0 || path.charAt(0) != '/';
    }

    /**
     * Returns a new Selector made of the concatenation of the specified prefix and
     * this Selector.
     * 
     * @param prefix the prefix to add.
     * @return a new Selector made of the prefix plus this Selector.
     */
    public Selector addPrefix(Path prefix) {
        return new Selector(prefix.toString() + this.path, this.filter, this.properties, this.fragment);
    }

}