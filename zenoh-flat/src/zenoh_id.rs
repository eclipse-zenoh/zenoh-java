use crate::ZZenohId;
use crate::z_zenoh_id_to_bytes;
use prebindgen_proc_macro::prebindgen;

/// Data-class twin of [`ZZenohId`]. Carries the raw bytes so bindings with
/// expensive FFI boundaries can read the id without round-tripping into
/// native code.
#[prebindgen]
#[derive(Clone)]
pub struct ZenohId {
    pub bytes: Vec<u8>,
}

impl From<&ZZenohId> for ZenohId {
    fn from(z: &ZZenohId) -> Self {
        Self {
            bytes: z_zenoh_id_to_bytes(z),
        }
    }
}

impl From<ZZenohId> for ZenohId {
    fn from(z: ZZenohId) -> Self {
        Self::from(&z)
    }
}
