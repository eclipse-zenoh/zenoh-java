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
use zenoh::{
    bytes::Encoding,
    config::Config,
    key_expr::KeyExpr,
    pubsub::Publisher,
    qos::{CongestionControl, Priority, Reliability},
    session::Session,
    Wait,
};

/// Open a Zenoh session using a borrowed configuration.
pub fn open_session(config: &Config) -> ZResult<Session> {
    zenoh::open(config.clone())
        .wait()
        .map_err(|err| zerror!(err))
}

/// Declare a publisher through an existing Zenoh session.
pub fn declare_publisher(
    session: &Session,
    key_expr: KeyExpr<'static>,
    congestion_control: CongestionControl,
    priority: Priority,
    express: bool,
    reliability: Reliability,
) -> ZResult<Publisher<'static>> {
    session
        .declare_publisher(key_expr)
        .congestion_control(congestion_control)
        .priority(priority)
        .express(express)
        .reliability(reliability)
        .wait()
        .map_err(|err| zerror!(err))
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
    reliability: Reliability,
    attachment: Option<Vec<u8>>,
) -> ZResult<()> {
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

    put_builder.wait().map_err(|err| zerror!(err)).map(|_| ())
}

/// Close a Zenoh session using a reference to the session.
pub fn close_session(session: &Session) -> ZResult<()> {
    session.close().wait().map_err(|err| zerror!(err))
}
