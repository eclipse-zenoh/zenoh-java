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
