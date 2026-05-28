use crate::{Encoding, KeyExpr, ReplyKeyExpr, ZBytes, ZQuery};
use prebindgen_proc_macro::prebindgen;

/// Data-class twin of [`ZQuery`]. Carries decoded query fields plus the
/// live [`ZQuery`] handle, which the binding uses to reply.
#[prebindgen]
pub struct Query {
    pub key_expr: KeyExpr,
    pub parameters: String,
    pub payload: Option<ZBytes>,
    pub encoding: Option<Encoding>,
    pub attachment: Option<ZBytes>,
    pub accepts_replies: ReplyKeyExpr,
    pub query: ZQuery,
}

impl From<ZQuery> for Query {
    fn from(q: ZQuery) -> Self {
        Query {
            key_expr: KeyExpr::from(q.key_expr().clone()),
            parameters: q.parameters().to_string(),
            payload: q.payload().map(|p| ZBytes::from(p.clone())),
            encoding: q.encoding().map(Encoding::from),
            attachment: q.attachment().map(|a| ZBytes::from(a.clone())),
            accepts_replies: q.accepts_replies().into(),
            query: q,
        }
    }
}

/// Consume an opaque [`ZQuery`] handle and repack it into the decoded
/// [`Query`] data class (re-embedding the same native query for replies).
#[prebindgen]
pub fn z_query_expand(query: ZQuery) -> Query {
    Query::from(query)
}
