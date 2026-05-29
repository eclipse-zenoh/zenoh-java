use crate::{CongestionControl, Encoding, KeyExpr, Priority, SampleKind, Timestamp, ZBytes, ZSample};
use prebindgen_proc_macro::prebindgen;

/// Data-class twin of [`ZSample`]. Carries every field decoded so bindings
/// with expensive FFI boundaries read the whole sample in one hop.
#[prebindgen]
#[derive(Clone)]
pub struct Sample {
    pub key_expr: KeyExpr,
    pub payload: ZBytes,
    pub encoding: Encoding,
    pub kind: SampleKind,
    pub timestamp: Option<Timestamp>,
    pub express: bool,
    pub priority: Priority,
    pub congestion_control: CongestionControl,
    pub attachment: Option<ZBytes>,
}

impl From<&ZSample> for Sample {
    fn from(s: &ZSample) -> Self {
        Sample {
            // String-only key expr: samples carry no declared keyexpr handle,
            // matching the legacy callback (KeyExpr built from the string).
            key_expr: KeyExpr::from(s.key_expr().to_string()),
            payload: ZBytes::from(s.payload().clone()),
            encoding: Encoding::from(s.encoding()),
            kind: s.kind().into(),
            timestamp: s.timestamp().map(Timestamp::from),
            express: s.express(),
            priority: s.priority().into(),
            congestion_control: s.congestion_control().into(),
            attachment: s.attachment().map(|a| ZBytes::from(a.clone())),
        }
    }
}
