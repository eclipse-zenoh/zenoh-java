use super::sample::Sample;
use crate::{CongestionControl, Priority, SampleKind, ZEncoding, ZKeyExpr, ZSample, ZTimestamp, ZZBytes};
use prebindgen_proc_macro::prebindgen;

/// Key expression the sample was published on (owned handle).
#[prebindgen]
pub fn z_sample_key_expr(s: &ZSample) -> ZKeyExpr {
    s.key_expr().clone()
}

/// Sample payload (owned bytes handle).
#[prebindgen]
pub fn z_sample_payload(s: &ZSample) -> ZZBytes {
    s.payload().clone()
}

/// Encoding of the payload (owned handle).
#[prebindgen]
pub fn z_sample_encoding(s: &ZSample) -> ZEncoding {
    s.encoding().clone()
}

/// Whether the sample is a PUT or a DELETE.
#[prebindgen]
pub fn z_sample_kind(s: &ZSample) -> SampleKind {
    s.kind().into()
}

/// Timestamp handle, or `None` when the sample carries no timestamp.
#[prebindgen]
pub fn z_sample_timestamp(s: &ZSample) -> Option<ZTimestamp> {
    s.timestamp().copied()
}

/// QoS express flag.
#[prebindgen]
pub fn z_sample_express(s: &ZSample) -> bool {
    s.express()
}

/// QoS priority.
#[prebindgen]
pub fn z_sample_priority(s: &ZSample) -> Priority {
    s.priority().into()
}

/// QoS congestion-control policy.
#[prebindgen]
pub fn z_sample_congestion_control(s: &ZSample) -> CongestionControl {
    s.congestion_control().into()
}

/// Optional user attachment (owned bytes handle), or `None`.
#[prebindgen]
pub fn z_sample_attachment(s: &ZSample) -> Option<ZZBytes> {
    s.attachment().cloned()
}

/// Decode a native [`ZSample`] into the thick [`Sample`] data class in one hop.
#[prebindgen]
pub fn z_sample_expand(s: &ZSample) -> Sample {
    Sample::from(s)
}
