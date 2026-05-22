// Copyright (c) 2023 ZettaScale Technology
//
// This program and the accompanying materials are made available under the
// terms of the Eclipse Public License 2.0 which is available at
// http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
// which is available at https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
//
// Contributors:
//   ZettaScale Zenoh Team, <zenoh@zettascale.tech>
//

//! Reply operations on an incoming [`Query`] exposed as `#[prebindgen]`
//! items.
//!
//! Each entry takes the [`Query`] by value: a single reply is allowed
//! and the query is dropped at the end of the call. Binding generators
//! that route by-value opaque-handle parameters through a consume
//! adapter (e.g. atomically invalidating the Java-side handle before
//! the Rust side reads it) get exactly-once semantics for free.

use crate::{errors::ZResult, zerror};
use prebindgen_proc_macro::prebindgen;
use tracing::{error, trace};
use zenoh::{
    bytes::Encoding,
    key_expr::KeyExpr as ZKeyExpr,
    query::Query,
    time::{Timestamp, TimestampId, NTP64},
    Wait,
};

// ── Flat query enums ──────────────────────────────────────────────────
// Mirror their upstream `zenoh::query::*` counterparts but are owned by
// `zenoh-flat`: discriminants are the wire values bindings send, and the
// `From` impls are the manual flat → upstream shim.

/// Flat mirror of [`zenoh::query::QueryTarget`].
#[prebindgen]
#[repr(i32)]
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Hash)]
pub enum QueryTarget {
    #[default]
    BestMatching = 0,
    All = 1,
    AllComplete = 2,
}

impl TryFrom<i32> for QueryTarget {
    type Error = String;

    fn try_from(v: i32) -> Result<Self, Self::Error> {
        Ok(match v {
            0 => QueryTarget::BestMatching,
            1 => QueryTarget::All,
            2 => QueryTarget::AllComplete,
            _ => return Err(format!("invalid QueryTarget discriminant: {}", v)),
        })
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

/// Flat mirror of [`zenoh::query::ConsolidationMode`].
#[prebindgen]
#[repr(i32)]
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Hash)]
pub enum ConsolidationMode {
    #[default]
    Auto = 0,
    None = 1,
    Monotonic = 2,
    Latest = 3,
}

impl TryFrom<i32> for ConsolidationMode {
    type Error = String;

    fn try_from(v: i32) -> Result<Self, Self::Error> {
        Ok(match v {
            0 => ConsolidationMode::Auto,
            1 => ConsolidationMode::None,
            2 => ConsolidationMode::Monotonic,
            3 => ConsolidationMode::Latest,
            _ => return Err(format!("invalid ConsolidationMode discriminant: {}", v)),
        })
    }
}

impl From<ConsolidationMode> for zenoh::query::ConsolidationMode {
    fn from(m: ConsolidationMode) -> Self {
        match m {
            ConsolidationMode::Auto => zenoh::query::ConsolidationMode::Auto,
            ConsolidationMode::None => zenoh::query::ConsolidationMode::None,
            ConsolidationMode::Monotonic => zenoh::query::ConsolidationMode::Monotonic,
            ConsolidationMode::Latest => zenoh::query::ConsolidationMode::Latest,
        }
    }
}

/// Flat mirror of [`zenoh::query::ReplyKeyExpr`]. Note the discriminants
/// follow the binding's wire order (`MatchingQuery = 0`, `Any = 1`),
/// which differs from upstream's variant declaration order — the `From`
/// impl maps by variant identity, not value.
#[prebindgen]
#[repr(i32)]
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Hash)]
pub enum ReplyKeyExpr {
    #[default]
    MatchingQuery = 0,
    Any = 1,
}

impl TryFrom<i32> for ReplyKeyExpr {
    type Error = String;

    fn try_from(v: i32) -> Result<Self, Self::Error> {
        Ok(match v {
            0 => ReplyKeyExpr::MatchingQuery,
            1 => ReplyKeyExpr::Any,
            _ => return Err(format!("invalid ReplyKeyExpr discriminant: {}", v)),
        })
    }
}

impl From<ReplyKeyExpr> for zenoh::query::ReplyKeyExpr {
    fn from(r: ReplyKeyExpr) -> Self {
        match r {
            ReplyKeyExpr::MatchingQuery => zenoh::query::ReplyKeyExpr::MatchingQuery,
            ReplyKeyExpr::Any => zenoh::query::ReplyKeyExpr::Any,
        }
    }
}

/// Reply with a successful sample to a [`Query`].
///
/// `timestamp_ntp64` carries an NTP64 value when `Some`; the reply's
/// timestamp ID is generated locally. The query is consumed.
#[prebindgen]
pub fn reply_success(
    query: Query,
    key_expr: impl Into<ZKeyExpr<'static>> + Send + 'static,
    payload: Vec<u8>,
    encoding: Encoding,
    timestamp_ntp64: Option<i64>,
    attachment: Option<Vec<u8>>,
    qos_express: bool,
) -> ZResult<()> {
    let ke: ZKeyExpr<'static> = key_expr.into();
    let mut reply_builder = query.reply(ke, payload).encoding(encoding);
    if let Some(ts) = timestamp_ntp64 {
        reply_builder = reply_builder.timestamp(Timestamp::new(NTP64(ts as u64), TimestampId::rand()));
    }
    if let Some(attachment) = attachment {
        reply_builder = reply_builder.attachment::<Vec<u8>>(attachment);
    }
    reply_builder
        .express(qos_express)
        .wait()
        .map(|_| trace!("Replied success to query."))
        .map_err(|err| {
            error!("Unable to reply success to query: {}", err);
            zerror!(err)
        })
}

/// Reply with an error to a [`Query`]. The query is consumed.
#[prebindgen]
pub fn reply_error(query: Query, payload: Vec<u8>, encoding: Encoding) -> ZResult<()> {
    query
        .reply_err(payload)
        .encoding(encoding)
        .wait()
        .map(|_| trace!("Replied error to query."))
        .map_err(|err| {
            error!("Unable to reply error to query: {}", err);
            zerror!(err)
        })
}

/// Reply with a delete to a [`Query`].
///
/// `timestamp_ntp64` carries an NTP64 value when `Some`; the reply's
/// timestamp ID is generated locally. The query is consumed.
#[prebindgen]
pub fn reply_delete(
    query: Query,
    key_expr: impl Into<ZKeyExpr<'static>> + Send + 'static,
    timestamp_ntp64: Option<i64>,
    attachment: Option<Vec<u8>>,
    qos_express: bool,
) -> ZResult<()> {
    let ke: ZKeyExpr<'static> = key_expr.into();
    let mut reply_builder = query.reply_del(ke);
    if let Some(ts) = timestamp_ntp64 {
        reply_builder = reply_builder.timestamp(Timestamp::new(NTP64(ts as u64), TimestampId::rand()));
    }
    if let Some(attachment) = attachment {
        reply_builder = reply_builder.attachment::<Vec<u8>>(attachment);
    }
    reply_builder
        .express(qos_express)
        .wait()
        .map(|_| trace!("Replied delete to query."))
        .map_err(|err| {
            error!("Unable to reply delete to query: {}", err);
            zerror!(err)
        })
}
