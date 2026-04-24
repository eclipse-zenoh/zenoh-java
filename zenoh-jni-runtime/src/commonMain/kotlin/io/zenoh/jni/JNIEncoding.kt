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

/**
 * JNI-boundary holder for a Zenoh encoding.
 *
 * Marshaled across JNI as a single object; the native side reads [id] and
 * [schema] via `env.get_field(...)`. Higher layers (e.g. `zenoh-java`'s
 * `io.zenoh.bytes.Encoding`) construct this wrapper at the JNI call
 * boundary. Keeping it here preserves the layering rule that
 * `zenoh-jni-runtime` must not depend on `zenoh-java`.
 */
data class JNIEncoding(val id: Int, val schema: String?)
