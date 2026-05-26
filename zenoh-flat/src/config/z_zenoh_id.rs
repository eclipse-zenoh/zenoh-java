use crate::ZZenohId;
use prebindgen_proc_macro::prebindgen;

/// Serialize a Zenoh node identifier as raw bytes (16 bytes, little-endian).
#[prebindgen]
pub fn z_zenoh_id_to_bytes(z: &ZZenohId) -> Vec<u8> {
    z.to_le_bytes().to_vec()
}

/// Format a Zenoh node identifier as its standard string form.
#[prebindgen]
pub fn z_zenoh_id_to_string(z: &ZZenohId) -> String {
    z.to_string()
}
