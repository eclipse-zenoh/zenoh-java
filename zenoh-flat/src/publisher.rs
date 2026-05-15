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

//! Publisher operations exposed to the JNI / Kotlin layer.
//!
//! These were previously hand-written `Java_io_zenoh_jni_JNIPublisher_*` JNI
//! functions in `zenoh-jni/src/publisher.rs`; they now live here as plain
//! `#[prebindgen]` Rust fns and are wrapped by the JNI generator into
//! `Java_io_zenoh_jni_JNINative_{put,delete,drop}PublisherViaJNI`.

use crate::{errors::ZResult, zerror};
use prebindgen_proc_macro::prebindgen;
use tracing::{error, trace};
use zenoh::{bytes::Encoding, pubsub::Publisher, Wait};

/// Publish a payload on an existing [`Publisher`].
///
/// `attachment` is appended to the publication when `Some`. The publisher
/// itself is borrowed — Java retains its strong `Arc` reference and may
/// continue to use the same handle for further put/delete calls.
#[prebindgen]
pub fn put_publisher(
    publisher: &Publisher<'static>,
    payload: Vec<u8>,
    encoding: Encoding,
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
    publisher: &Publisher<'static>,
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

// `drop_publisher` is intentionally NOT migrated here. The current
// JniExt input convention for opaque Arc handles only supports
// borrow-style use (it forgets the inner `Arc` on drop, leaving the
// outer strong count untouched). A drop fn must actually decrement the
// outer Arc — that's done via `Arc::from_raw(v)` in the hand-written
// `Java_io_zenoh_jni_JNIPublisher_freePtrViaJNI` over in
// `zenoh-jni/src/publisher.rs`. Migrating it cleanly will require a
// separate "consume" input convention (or a follow-up that fixes
// `drop_session`'s analogous leak).
