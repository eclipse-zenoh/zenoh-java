use prebindgen_proc_macro::prebindgen;

/// Whether a sample is a PUT or a DELETE. Mirrors `zenoh::sample::SampleKind`.
#[prebindgen]
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleKind {
    Put = 0,
    Delete = 1,
}

impl From<zenoh::sample::SampleKind> for SampleKind {
    fn from(k: zenoh::sample::SampleKind) -> Self {
        match k {
            zenoh::sample::SampleKind::Put => SampleKind::Put,
            zenoh::sample::SampleKind::Delete => SampleKind::Delete,
        }
    }
}
