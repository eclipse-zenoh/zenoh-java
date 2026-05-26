use crate::{WhatAmI, ZHello, ZZenohId};
use prebindgen_proc_macro::prebindgen;

/// Node type that emitted this hello message.
#[prebindgen]
pub fn z_hello_whatami(h: &ZHello) -> WhatAmI {
    h.whatami().into()
}

/// Zenoh id of the node that emitted this hello message.
#[prebindgen]
pub fn z_hello_zid(h: &ZHello) -> ZZenohId {
    h.zid()
}

/// Locators advertised in this hello message.
#[prebindgen]
pub fn z_hello_locators(h: &ZHello) -> Vec<String> {
    h.locators().iter().map(|l| l.to_string()).collect()
}
