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

import io.zenoh.bytes.Encoding
import io.zenoh.bytes.IntoZBytes
import io.zenoh.bytes.ZBytes
import java.time.Duration

/**
 * Get to query data from the matching queryables in the system.
 *
 * @param timeout Timeout of the query.
 * @param target The [QueryTarget] of the query.
 * @param consolidation The [ConsolidationMode] of the query.
 * @param payload Optional payload.
 * @param encoding Encoding of the payload.
 * @param attachment Optional attachment.
 */
data class GetOptions(
    var timeout: Duration = Duration.ofMillis(10000),
    var target: QueryTarget = QueryTarget.BEST_MATCHING,
    var consolidation: ConsolidationMode = ConsolidationMode.AUTO,
    var payload: IntoZBytes? = null,
    var encoding: Encoding? = null,
    var attachment: IntoZBytes? = null
) {
    fun setPayload(payload: String) = apply { this.payload = ZBytes.from(payload) }
    fun setAttachment(attachment: String) = apply { this.attachment = ZBytes.from(attachment) }
}
