use super::timestamp::Timestamp;
use crate::ZTimestamp;
use prebindgen_proc_macro::prebindgen;

/// NTP64 time component of the timestamp.
#[prebindgen]
pub fn z_timestamp_ntp64(t: &ZTimestamp) -> i64 {
    t.get_time().as_u64() as i64
}

/// Raw bytes of the originating `TimestampId`.
#[prebindgen]
pub fn z_timestamp_id(t: &ZTimestamp) -> Vec<u8> {
    let id = t.get_id();
    id.to_le_bytes()[..id.size()].to_vec()
}

/// Decode a native [`ZTimestamp`] into the thick [`Timestamp`] data class in one hop.
#[prebindgen]
pub fn z_timestamp_expand(t: &ZTimestamp) -> Timestamp {
    Timestamp::from(t)
}
