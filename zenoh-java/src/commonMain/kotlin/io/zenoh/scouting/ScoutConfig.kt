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

package io.zenoh.scouting

import io.zenoh.Config
import io.zenoh.config.WhatAmI

data class ScoutConfig(
    var config: Config? = null,
    var whatAmI: Set<WhatAmI> = setOf(WhatAmI.Peer, WhatAmI.Router)
) {

    fun config(config: Config) = apply { this.config = config }

    fun whatAmI(whatAmI: Set<WhatAmI>) = apply { this.whatAmI = whatAmI }
}
