use crate::ZZBytes;
use prebindgen_proc_macro::prebindgen;

/// Read the payload bytes carried by a native [`ZZBytes`]. Performs one
/// copy if the underlying buffer is non-contiguous (mirrors
/// `zenoh::bytes::ZBytes::to_bytes`).
#[prebindgen]
pub fn z_zbytes_to_bytes(z: &ZZBytes) -> Vec<u8> {
    z.to_bytes().into_owned()
}

/// Construct a native [`ZZBytes`] from a raw byte buffer.
#[prebindgen]
pub fn z_zbytes_from_bytes(bytes: Vec<u8>) -> ZZBytes {
    ZZBytes::from(bytes)
}
