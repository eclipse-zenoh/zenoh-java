use prebindgen_proc_macro::prebindgen;

/// The kind of consolidation applied to query replies. Mirrors
/// `zenoh::query::ConsolidationMode`.
#[prebindgen]
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsolidationMode {
    Auto = 0,
    None = 1,
    Monotonic = 2,
    Latest = 3,
}

impl From<zenoh::query::ConsolidationMode> for ConsolidationMode {
    fn from(c: zenoh::query::ConsolidationMode) -> Self {
        match c {
            zenoh::query::ConsolidationMode::Auto => ConsolidationMode::Auto,
            zenoh::query::ConsolidationMode::None => ConsolidationMode::None,
            zenoh::query::ConsolidationMode::Monotonic => ConsolidationMode::Monotonic,
            zenoh::query::ConsolidationMode::Latest => ConsolidationMode::Latest,
        }
    }
}

impl From<ConsolidationMode> for zenoh::query::ConsolidationMode {
    fn from(c: ConsolidationMode) -> Self {
        match c {
            ConsolidationMode::Auto => zenoh::query::ConsolidationMode::Auto,
            ConsolidationMode::None => zenoh::query::ConsolidationMode::None,
            ConsolidationMode::Monotonic => zenoh::query::ConsolidationMode::Monotonic,
            ConsolidationMode::Latest => zenoh::query::ConsolidationMode::Latest,
        }
    }
}
