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

import io.zenoh.jni.QueryTarget as JniQueryTarget

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

    /** Project onto the JNI-layer enum that crosses the boundary. */
    fun toJni(): JniQueryTarget = when (this) {
        BEST_MATCHING -> JniQueryTarget.BEST_MATCHING
        ALL -> JniQueryTarget.ALL
        ALL_COMPLETE -> JniQueryTarget.ALL_COMPLETE
    }
}

