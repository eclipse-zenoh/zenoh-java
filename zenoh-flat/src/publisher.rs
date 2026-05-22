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

//! Publisher operations exposed as `#[prebindgen]` items for downstream
//! binding generators.

use crate::{errors::ZResult, zerror};
use prebindgen_proc_macro::prebindgen;
use tracing::{error, trace};
use zenoh::{bytes::Encoding as ZEncoding, pubsub::Publisher as ZPublisher, Wait as ZWait};

/// Publish a payload on an existing [`Publisher`].
///
/// `attachment` is appended to the publication when `Some`. The publisher
/// itself is borrowed — the caller retains its handle and may continue to
/// use it for further put/delete calls.
#[prebindgen]
pub fn put_publisher(
    publisher: &ZPublisher<'static>,
    payload: Vec<u8>,
    encoding: ZEncoding,
    attachment: Option<Vec<u8>>,
) -> ZResult<()> {
    let mut publication = publisher.put(payload).encoding(encoding);
    if let Some(attachment) = attachment {
        publication = publication.attachment::<Vec<u8>>(attachment);
    }
    publication
        .wait()
        .map(|_| trace!("Published on publisher."))
        .map_err(|err| {
            error!("Unable to put on publisher: {}", err);
            zerror!(err)
        })
}

/// Delete on an existing [`Publisher`].
#[prebindgen]
pub fn delete_publisher(
    publisher: &ZPublisher<'static>,
    attachment: Option<Vec<u8>>,
) -> ZResult<()> {
    let mut delete = publisher.delete();
    if let Some(attachment) = attachment {
        delete = delete.attachment::<Vec<u8>>(attachment);
    }
    delete
        .wait()
        .map(|_| trace!("Deleted on publisher."))
        .map_err(|err| {
            error!("Unable to delete on publisher: {}", err);
            zerror!(err)
        })
}

