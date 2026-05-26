use prebindgen_proc_macro::prebindgen;

/// Node type in a Zenoh network: router, peer, or client. The discriminant
/// values match upstream `zenoh::config::WhatAmI` (1, 2, 4) so an OR'd
/// bitfield of variants encodes a `WhatAmIMatcher` directly as `u8`.
#[prebindgen]
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhatAmI {
    Router = 1,
    Peer = 2,
    Client = 4,
}

impl From<zenoh::config::WhatAmI> for WhatAmI {
    fn from(w: zenoh::config::WhatAmI) -> Self {
        match w {
            zenoh::config::WhatAmI::Router => WhatAmI::Router,
            zenoh::config::WhatAmI::Peer => WhatAmI::Peer,
            zenoh::config::WhatAmI::Client => WhatAmI::Client,
        }
    }
}
