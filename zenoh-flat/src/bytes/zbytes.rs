use crate::ZZBytes;
use super::z_zbytes::z_zbytes_to_bytes;
use prebindgen_proc_macro::prebindgen;

/// Value-class twin of [`ZZBytes`]. Carries the payload bytes so
/// bindings with expensive FFI boundaries can read them without
/// round-tripping into native code.
#[prebindgen]
#[derive(Clone)]
pub struct ZBytes {
    pub bytes: Vec<u8>,
}

impl From<&ZZBytes> for ZBytes {
    fn from(z: &ZZBytes) -> Self {
        Self {
            bytes: z_zbytes_to_bytes(z),
        }
    }
}

impl From<ZZBytes> for ZBytes {
    fn from(z: ZZBytes) -> Self {
        Self::from(&z)
    }
}

impl From<ZBytes> for ZZBytes {
    fn from(z: ZBytes) -> Self {
        ZZBytes::from(z.bytes)
    }
}

impl From<&ZBytes> for ZZBytes {
    fn from(z: &ZBytes) -> Self {
        ZZBytes::from(&z.bytes)
    }
}

impl From<Vec<u8>> for ZBytes {
    fn from(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }
}
