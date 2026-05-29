use prebindgen_proc_macro::prebindgen;

/// Whether a query accepts replies whose key expression may not match the
/// query's. Mirrors `zenoh::query::ReplyKeyExpr`.
#[prebindgen]
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplyKeyExpr {
    Any = 0,
    MatchingQuery = 1,
}

impl From<zenoh::query::ReplyKeyExpr> for ReplyKeyExpr {
    fn from(r: zenoh::query::ReplyKeyExpr) -> Self {
        match r {
            zenoh::query::ReplyKeyExpr::Any => ReplyKeyExpr::Any,
            zenoh::query::ReplyKeyExpr::MatchingQuery => ReplyKeyExpr::MatchingQuery,
        }
    }
}

impl From<ReplyKeyExpr> for zenoh::query::ReplyKeyExpr {
    fn from(r: ReplyKeyExpr) -> Self {
        match r {
            ReplyKeyExpr::Any => zenoh::query::ReplyKeyExpr::Any,
            ReplyKeyExpr::MatchingQuery => zenoh::query::ReplyKeyExpr::MatchingQuery,
        }
    }
}
