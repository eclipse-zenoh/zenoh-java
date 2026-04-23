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

use crate::{errors::ZResult, zerror};
use tracing::{error, trace};
use std::time::Duration;

use zenoh::{
    bytes::Encoding,
    config::Config,
    key_expr::KeyExpr,
    pubsub::{Publisher, Subscriber},
    query::{ConsolidationMode, Query, QueryTarget, Queryable, Querier, ReplyKeyExpr},
    sample::Sample,
    qos::{CongestionControl, Priority, Reliability},
    session::Session,
    Wait,
};

/// Open a Zenoh session using a borrowed configuration.
#[prebindgen_proc_macro::prebindgen("jni")]
pub fn open_session(config: &Config) -> ZResult<Session> {
    zenoh::open(config.clone())
        .wait()
        .map(|session| {
            trace!("Opened Zenoh session.");
            session
        })
        .map_err(|err| {
            error!("Unable to open session: {}", err);
            zerror!(err)
        })
}

/// Declare a publisher through an existing Zenoh session.
#[prebindgen_proc_macro::prebindgen("jni")]
pub fn declare_publisher(
    session: &Session,
    key_expr: KeyExpr<'static>,
    congestion_control: CongestionControl,
    priority: Priority,
    express: bool,
    reliability: Reliability,
) -> ZResult<Publisher<'static>> {
    let key_expr_string = key_expr.to_string();
    session
        .declare_publisher(key_expr)
        .congestion_control(congestion_control)
        .priority(priority)
        .express(express)
        .reliability(reliability)
        .wait()
        .map(|publisher| {
            trace!("Declared publisher on '{}'.", key_expr_string);
            publisher
        })
        .map_err(|err| {
            error!("Unable to declare publisher on '{}': {}", key_expr_string, err);
            zerror!(err)
        })
}

/// Declare a subscriber through an existing Zenoh session.
pub fn declare_subscriber(
    session: &Session,
    key_expr: KeyExpr<'static>,
    callback: impl Fn(Sample) + Send + Sync + 'static,
) -> ZResult<Subscriber<()>> {
    let key_expr_string = key_expr.to_string();
    session
        .declare_subscriber(key_expr)
        .callback(callback)
        .wait()
        .map(|subscriber| {
            trace!("Declared subscriber on '{}'.", key_expr_string);
            subscriber
        })
        .map_err(|err| {
            error!("Unable to declare subscriber on '{}': {}", key_expr_string, err);
            zerror!(err)
        })
}

/// Declare a querier through an existing Zenoh session.
#[prebindgen_proc_macro::prebindgen("jni")]
pub fn declare_querier(
    session: &Session,
    key_expr: KeyExpr<'static>,
    query_target: QueryTarget,
    consolidation: ConsolidationMode,
    congestion_control: CongestionControl,
    priority: Priority,
    express: bool,
    timeout: Duration,
    reply_key_expr: ReplyKeyExpr,
) -> ZResult<Querier<'static>> {
    let key_expr_string = key_expr.to_string();
    session
        .declare_querier(key_expr)
        .congestion_control(congestion_control)
        .consolidation(consolidation)
        .express(express)
        .target(query_target)
        .priority(priority)
        .timeout(timeout)
        .accept_replies(reply_key_expr)
        .wait()
        .map(|querier| {
            trace!("Declared querier on '{}'.", key_expr_string);
            querier
        })
        .map_err(|err| {
            error!("Unable to declare querier on '{}': {}", key_expr_string, err);
            zerror!(err)
        })
}

/// Declare a queryable through an existing Zenoh session.
pub fn declare_queryable(
    session: &Session,
    key_expr: KeyExpr<'static>,
    callback: impl Fn(Query) + Send + Sync + 'static,
    complete: bool,
) -> ZResult<Queryable<()>> {
    let key_expr_string = key_expr.to_string();
    session
        .declare_queryable(key_expr)
        .callback(callback)
        .complete(complete)
        .wait()
        .map(|queryable| {
            trace!("Declared queryable on '{}'.", key_expr_string);
            queryable
        })
        .map_err(|err| {
            error!("Unable to declare queryable on '{}': {}", key_expr_string, err);
            zerror!(err)
        })
}

/// Perform a put operation through an existing Zenoh session.
pub fn put(
    session: &Session,
    key_expr: KeyExpr<'static>,
    payload: Vec<u8>,
    encoding: Encoding,
    congestion_control: CongestionControl,
    priority: Priority,
    express: bool,
    attachment: Option<Vec<u8>>,
    reliability: Reliability,
) -> ZResult<()> {
    let key_expr_string = key_expr.to_string();
    let mut put_builder = session
        .put(&key_expr, payload)
        .congestion_control(congestion_control)
        .encoding(encoding)
        .express(express)
        .priority(priority)
        .reliability(reliability);

    if let Some(attachment) = attachment {
        put_builder = put_builder.attachment(attachment);
    }

    put_builder
        .wait()
        .map(|_| {
            trace!("Put on '{}'.", key_expr_string);
        })
        .map_err(|err| {
            error!("Unable to put on '{}': {}", key_expr_string, err);
            zerror!(err)
        })
}

/// Perform a delete operation through an existing Zenoh session.
#[prebindgen_proc_macro::prebindgen("jni")]
pub fn delete(
    session: &Session,
    key_expr: KeyExpr<'static>,
    congestion_control: CongestionControl,
    priority: Priority,
    express: bool,
    attachment: Option<Vec<u8>>,
    reliability: Reliability,
) -> ZResult<()> {
    let key_expr_string = key_expr.to_string();
    let mut delete_builder = session
        .delete(&key_expr)
        .congestion_control(congestion_control)
        .express(express)
        .priority(priority)
        .reliability(reliability);

    if let Some(attachment) = attachment {
        delete_builder = delete_builder.attachment(attachment);
    }

    delete_builder
        .wait()
        .map(|_| {
            trace!("Delete on '{}'.", key_expr_string);
        })
        .map_err(|err| {
            error!("Unable to delete on '{}': {}", key_expr_string, err);
            zerror!(err)
        })
}

/// Close a Zenoh session using a reference to the session.
pub fn close_session(session: &Session) -> ZResult<()> {
    session
        .close()
        .wait()
        .map(|_| {
            trace!("Closed Zenoh session.");
        })
        .map_err(|err| {
            error!("Unable to close session: {}", err);
            zerror!(err)
        })
}
