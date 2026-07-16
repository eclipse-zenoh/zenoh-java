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

package io.zenoh.query

/** The Queryables that should be targeted by a GET operation. */
enum class QueryTarget {

    /**
     * Best Matching: the nearest complete queryable if any else all matching queryables.
     */
    BEST_MATCHING,

    /**
     * All matching queryables.
     */
    ALL,

    /**
     * All Complete queryables.
     */
    ALL_COMPLETE;
}

internal fun QueryTarget.toFlat(): io.zenoh.jni.query.QueryTarget = when (this) {
    QueryTarget.BEST_MATCHING -> io.zenoh.jni.query.QueryTarget.BEST_MATCHING
    QueryTarget.ALL -> io.zenoh.jni.query.QueryTarget.ALL
    QueryTarget.ALL_COMPLETE -> io.zenoh.jni.query.QueryTarget.ALL_COMPLETE
}

