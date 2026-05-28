use crate::ZTimestamp;
use prebindgen_proc_macro::prebindgen;

/// Data-class twin of [`ZTimestamp`]. Carries the full zenoh timestamp:
/// the NTP64 time and the originating `TimestampId` bytes.
#[prebindgen]
#[derive(Clone)]
pub struct Timestamp {
    pub ntp64: i64,
    pub id: Vec<u8>,
}

impl From<&ZTimestamp> for Timestamp {
    fn from(t: &ZTimestamp) -> Self {
        let id = t.get_id();
        Timestamp {
            ntp64: t.get_time().as_u64() as i64,
            id: id.to_le_bytes()[..id.size()].to_vec(),
        }
    }
}
