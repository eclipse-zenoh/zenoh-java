use prebindgen_proc_macro::prebindgen;

/// The queryables that should be targeted by a GET. Mirrors
/// `zenoh::query::QueryTarget`.
#[prebindgen]
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryTarget {
    BestMatching = 0,
    All = 1,
    AllComplete = 2,
}

impl From<zenoh::query::QueryTarget> for QueryTarget {
    fn from(t: zenoh::query::QueryTarget) -> Self {
        match t {
            zenoh::query::QueryTarget::BestMatching => QueryTarget::BestMatching,
            zenoh::query::QueryTarget::All => QueryTarget::All,
            zenoh::query::QueryTarget::AllComplete => QueryTarget::AllComplete,
        }
    }
}

impl From<QueryTarget> for zenoh::query::QueryTarget {
    fn from(t: QueryTarget) -> Self {
        match t {
            QueryTarget::BestMatching => zenoh::query::QueryTarget::BestMatching,
            QueryTarget::All => zenoh::query::QueryTarget::All,
            QueryTarget::AllComplete => zenoh::query::QueryTarget::AllComplete,
        }
    }
}
