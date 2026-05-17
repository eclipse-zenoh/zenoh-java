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
import java.util.concurrent.locks.ReentrantReadWriteLock
import kotlin.concurrent.read
import kotlin.concurrent.write

/**
 * Race-free wrapper around a raw `Arc<T>` pointer obtained from native
 * code via `Arc::into_raw(Arc::new(v))`. Pairs the pointer with a
 * `ReentrantReadWriteLock` so that:
 *
 *  * Concurrent borrow-style JNI calls ([withPtr]) run in parallel
 *    under the read lock and see a non-zero pointer for the duration
 *    of the call.
 *  * [close] takes the write lock, drains any in-flight borrows, takes
 *    the pointer atomically, and invokes the supplied free function
 *    exactly once.
 *
 * This is the JVM-side half of SAFETY_ANALYSIS.md Variant C: it
 * sequences each borrow's `Arc::increment_strong_count` strictly
 * before any free's `Arc::from_raw` + drop on the same allocation,
 * closing the use-after-free window that exists when the field is just
 * a plain `var Long`.
 */
public class NativeHandle(initial: Long) {
    private val lock = ReentrantReadWriteLock()
    private var ptr: Long = initial

    /**
     * Run [block] with the live pointer under the read lock. Throws
     * [ZError] if [close] has already released the handle. Multiple
     * concurrent invocations run in parallel; only [close] is
     * serialized against them.
     */
    @Throws(ZError::class)
    public fun <T> withPtr(block: (Long) -> T): T = lock.read {
        val p = ptr
        if (p == 0L) throw ZError("Operation on a closed native handle.")
        block(p)
    }

    /**
     * Take the pointer under the write lock and pass it to [freeFn]
     * exactly once. Subsequent [close] calls are no-ops. Blocks until
     * all in-flight [withPtr] calls finish.
     */
    public fun close(freeFn: (Long) -> Unit) {
        lock.write {
            val p = ptr
            if (p == 0L) return@write
            ptr = 0L
            freeFn(p)
        }
    }

    /** True iff [close] has run. */
    public fun isClosed(): Boolean = lock.read { ptr == 0L }
}
