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

/**
 * A zenoh Path is a set of strings separated by '/' , as in a filesystem path.
 * A Path cannot contain any '*' character. Examples of paths:
 * "/demo/example/test" , "/com/adlink/building/fr/floor/1/office/2" ...
 * <p>
 * A path can be absolute (i.e. starting with a `'/'`) or relative to a
 * {@link Workspace}.
 *
 * @see Workspace#put(Path, Value)
 */
public class Path implements Comparable<Path> {

    private String path;

    /**
     * Create a Path from a string such as "/demo/example/test".
     *
     * @param p the string
     */
    public Path(String p) {
        if (p == null) {
            throw new NullPointerException("The given path is null");
        }
        if (p.isEmpty()) {
            throw new IllegalArgumentException("Invalid path (empty String)");
        }
        for (int i = 0; i < p.length(); ++i) {
            char c = p.charAt(i);
            if (c == '?' || c == '#' || c == '[' || c == ']' || c == '*')
                throw new IllegalArgumentException("Invalid path: " + p + " (forbidden character at index " + i + ")");
        }
        this.path = removeUselessSlashes(p);
    }

    private String removeUselessSlashes(String s) {
        String result = s.replaceAll("/+", "/");
        if (result.charAt(result.length() - 1) == '/') {
            return result.substring(0, result.length() - 1);
        } else {
            return result;
        }
    }

    @Override
    public String toString() {
        return path;
    }

    @Override
    public int compareTo(Path p) {
        return this.path.compareTo(p.path);
    }

    @Override
    public boolean equals(Object object) {
        if (object == null) {
            return false;
        }
        if (this == object) {
            return true;
        }
        if (object instanceof Path) {
            return this.path.equals(((Path) object).path);
        }
        return false;
    }

    @Override
    public int hashCode() {
        return path.hashCode();
    }

    /**
     * The Path length (i.e. the length of the Path as a string)
     * 
     * @return the length
     */
    public int length() {
        return path.length();
    }

    /**
     * Returns true if the Path is relative (i.e. it doesn't start with '/')
     * 
     * @return true if the Path is relative.
     */
    public boolean isRelative() {
        return length() == 0 || path.charAt(0) != '/';
    }

    /**
     * Returns a new Path made of the concatenation of the specified prefix and this
     * Path.
     * 
     * @param prefix the prefix to add.
     * @return a new Path made of the prefix plus this Path.
     */
    public Path addPrefix(Path prefix) {
        return new Path(prefix.toString() + "/" + this.path);
    }

}
