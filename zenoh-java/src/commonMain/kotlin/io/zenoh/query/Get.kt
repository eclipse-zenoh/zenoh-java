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
import java.time.Duration

/**
 * Get to query data from the matching queryables in the system.
 *
 * TODO
 */
data class GetConfig(
    var timeout: Duration = Duration.ofMillis(10000),
    var target: QueryTarget = QueryTarget.BEST_MATCHING,
    var consolidation: ConsolidationMode = ConsolidationMode.AUTO,
    var payload: IntoZBytes? = null,
    var encoding: Encoding? = null,
    var attachment: IntoZBytes? = null,
    var onClose: Runnable? = null,
) {

    /** Specify the [QueryTarget]. */
    fun target(target: QueryTarget) = apply {
        this.target = target
    }

    /** Specify the [ConsolidationMode]. */
    fun consolidation(consolidation: ConsolidationMode) = apply {
        this.consolidation = consolidation
    }

    /** Specify the timeout. */
    fun timeout(timeout: Duration) = apply {
        this.timeout = timeout
    }

    fun payload(payload: IntoZBytes) = apply {
        this.payload = payload.into()
    }

    fun encoding(encoding: Encoding) = apply {
        this.encoding = encoding
    }

    /** Specify an attachment. */
    fun attachment(attachment: IntoZBytes) = apply {
        this.attachment = attachment.into()
    }

    /**
     * Specify an action to be invoked when the Get operation is over.
     *
     * Zenoh will trigger ths specified action once no more replies are to be expected.
     */
    fun onClose(action: Runnable) = apply {
        this.onClose = action
    }
}
