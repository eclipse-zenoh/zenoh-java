use crate::{WhatAmI, ZHello, ZenohId};
use super::z_hello::{z_hello_locators, z_hello_whatami, z_hello_zid};
use prebindgen_proc_macro::prebindgen;

/// Data-class twin of [`ZHello`]. Carries decomposed fields so bindings
/// with expensive FFI boundaries can read the hello payload without a
/// round-trip per field.
#[prebindgen]
pub struct Hello {
    pub whatami: WhatAmI,
    pub zid: ZenohId,
    pub locators: Vec<String>,
}

impl From<&ZHello> for Hello {
    fn from(h: &ZHello) -> Self {
        Self {
            whatami: z_hello_whatami(h),
            zid: ZenohId::from(z_hello_zid(h)),
            locators: z_hello_locators(h),
        }
    }
}

impl From<ZHello> for Hello {
    fn from(h: ZHello) -> Self {
        Self::from(&h)
    }
}
