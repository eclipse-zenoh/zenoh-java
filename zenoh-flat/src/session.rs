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

use crate::zerror;
use zenoh::{config::Config, session::Session, Wait};

/// Open a Zenoh session using a borrowed configuration.
pub fn open_session(config: &Config) -> crate::errors::ZResult<Session> {
    zenoh::open(config.clone())
        .wait()
        .map_err(|err| zerror!(err))
}

/// Close a Zenoh session using a reference to the session.
pub fn close_session(session: &Session) -> crate::errors::ZResult<()> {
    session.close().wait().map_err(|err| zerror!(err))
}
