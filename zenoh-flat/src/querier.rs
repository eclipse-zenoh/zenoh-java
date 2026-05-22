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

//! Querier `get` operations exposed as `#[prebindgen]` items.

use crate::{errors::ZResult, zerror};
use prebindgen_proc_macro::prebindgen;
use tracing::{error, trace};
use zenoh::{
    bytes::Encoding as ZEncoding,
    query::{Querier as ZQuerier, Reply as ZReply},
    Wait as ZWait,
};

/// Fires `f` exactly once when dropped. Bound to the reply callback so
/// teardown of the closure (query completion) runs the user's
/// `on_close`.
struct CallOnDrop<F: FnOnce()>(core::mem::MaybeUninit<F>);
impl<F: FnOnce()> CallOnDrop<F> {
    fn new(f: F) -> Self {
        Self(core::mem::MaybeUninit::new(f))
    }
}
impl<F: FnOnce()> Drop for CallOnDrop<F> {
    fn drop(&mut self) {
        let f = unsafe { self.0.assume_init_read() };
        f();
    }
}

/// Perform a get through an existing [`Querier`]. The querier's
/// declared key expression is used.
///
/// `parameters` is appended to the querier's selector as query
/// parameters when `Some`. `payload` and `encoding` are coupled:
/// passing a payload attaches the encoding; without a payload the
/// encoding is ignored.
#[prebindgen]
pub fn querier_get(
    querier: &ZQuerier<'static>,
    parameters: Option<String>,
    callback: impl Fn(ZReply) + Send + Sync + 'static,
    on_close: impl Fn() + Send + Sync + 'static,
    attachment: Option<Vec<u8>>,
    payload: Option<Vec<u8>>,
    encoding: Option<ZEncoding>,
) -> ZResult<()> {
    let guard = CallOnDrop::new(on_close);
    let mut get_builder = querier.get().callback(move |reply| {
        let _ = &guard;
        callback(reply);
    });

    if let Some(params) = parameters {
        get_builder = get_builder.parameters(params);
    }

    if let Some(payload) = payload {
        if let Some(encoding) = encoding {
            get_builder = get_builder.encoding(encoding);
        }
        get_builder = get_builder.payload(payload);
    }

    if let Some(attachment) = attachment {
        get_builder = get_builder.attachment::<Vec<u8>>(attachment);
    }

    get_builder
        .wait()
        .map(|_| trace!("Performing querier get."))
        .map_err(|err| {
            error!("Unable to perform querier get: {}", err);
            zerror!(err)
        })
}
