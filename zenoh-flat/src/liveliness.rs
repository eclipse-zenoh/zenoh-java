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

//! Liveliness operations exposed as `#[prebindgen]` items.

use std::time::Duration;

use crate::sample::Sample;
use crate::{errors::ZResult, zerror};
use prebindgen_proc_macro::prebindgen;
use tracing::{error, trace};
use zenoh::{
    key_expr::KeyExpr as ZKeyExpr,
    liveliness::LivelinessToken as ZLivelinessToken,
    pubsub::Subscriber as ZSubscriber,
    query::Reply as ZReply,
    session::Session as ZSession,
    Wait as ZWait,
};

/// Fires `f` exactly once when dropped. Bound to the data callback so
/// teardown of the closure (subscription cancellation, get completion)
/// runs the user's `on_close`.
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

/// Declare a [`LivelinessToken`] on `session` for `key_expr`. The
/// returned token's liveliness is monitored by remote subscribers
/// until it is dropped.
#[prebindgen]
pub fn declare_liveliness_token(
    session: &ZSession,
    key_expr: impl Into<ZKeyExpr<'static>> + Send + 'static,
) -> ZResult<ZLivelinessToken> {
    let ke: ZKeyExpr<'static> = key_expr.into();
    let key_expr_string = ke.to_string();
    session
        .liveliness()
        .declare_token(ke)
        .wait()
        .inspect(|_| trace!("Declared liveliness token on '{}'.", key_expr_string))
        .map_err(|err| {
            error!(
                "Unable to declare liveliness token on '{}': {}",
                key_expr_string, err
            );
            zerror!(err)
        })
}

/// Declare a subscriber that observes liveliness events matching
/// `key_expr`. When `history` is true, the subscriber receives an
/// initial event for each currently-alive matching token.
#[prebindgen]
pub fn declare_liveliness_subscriber(
    session: &ZSession,
    key_expr: impl Into<ZKeyExpr<'static>> + Send + 'static,
    callback: impl Fn(Sample) + Send + Sync + 'static,
    on_close: impl Fn() + Send + Sync + 'static,
    history: bool,
) -> ZResult<ZSubscriber<()>> {
    let ke: ZKeyExpr<'static> = key_expr.into();
    let key_expr_string = ke.to_string();
    let guard = CallOnDrop::new(on_close);
    session
        .liveliness()
        .declare_subscriber(ke)
        .history(history)
        .callback(move |zsample| {
            let _ = &guard;
            callback((&zsample).into());
        })
        .wait()
        .inspect(|_| trace!("Declared liveliness subscriber on '{}'.", key_expr_string))
        .map_err(|err| {
            error!(
                "Unable to declare liveliness subscriber on '{}': {}",
                key_expr_string, err
            );
            zerror!(err)
        })
}

/// Query the liveliness tokens whose key expressions match `key_expr`.
/// Each [`Reply`] is delivered to `callback`; `on_close` runs once the
/// query session completes (either timeout or cancellation).
#[prebindgen]
pub fn liveliness_get(
    session: &ZSession,
    key_expr: impl Into<ZKeyExpr<'static>> + Send + 'static,
    callback: impl Fn(ZReply) + Send + Sync + 'static,
    on_close: impl Fn() + Send + Sync + 'static,
    timeout: Duration,
) -> ZResult<()> {
    let ke: ZKeyExpr<'static> = key_expr.into();
    let key_expr_string = ke.to_string();
    let guard = CallOnDrop::new(on_close);
    session
        .liveliness()
        .get(ke)
        .timeout(timeout)
        .callback(move |reply| {
            let _ = &guard;
            callback(reply);
        })
        .wait()
        .map(|_| trace!("Performing liveliness get on '{}'.", key_expr_string))
        .map_err(|err| {
            error!(
                "Unable to perform liveliness get on '{}': {}",
                key_expr_string, err
            );
            zerror!(err)
        })
}
