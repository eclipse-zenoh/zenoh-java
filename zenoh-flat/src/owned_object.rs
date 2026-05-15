//
// Copyright (c) 2026 ZettaScale Technology
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

//! Borrow-without-decrement wrapper for raw [`Arc`] pointers.
//!
//! Used by the JNI bindings to give a `#[prebindgen]` fn a value it can
//! treat as an owned `T` (via [`Deref`]) without consuming the strong
//! reference held by the Java side. On drop the inner `Arc` is forgotten
//! — Java's refcount is never touched by per-call decoding.
//!
//! The JNI plugin's `opaque_arc_borrow_input` helper produces converters
//! whose return type is `OwnedObject<T>`. Function signatures of the form
//! `&T` work transparently because `&OwnedObject<T>` auto-derefs to `&T`.
//!
//! For types where `T: Clone`, the simpler [`std::sync::Arc::clone`]
//! pattern (used by `JniExt::opaque_arc_input`) avoids constructing
//! `OwnedObject` at all. This wrapper exists for the non-`Clone` case
//! (e.g. zenoh's `Publisher<'a>`) where an owned-by-value `T` is not
//! reachable from the wire pointer without consuming the outer `Arc`.

use std::{ops::Deref, sync::Arc};

/// Safe accessor to refcounted ([`Arc`]) owned objects.
///
/// Helps avoid early drop by handling [`std::mem::forget`] internally.
pub struct OwnedObject<T: ?Sized> {
    inner: Option<Arc<T>>,
}

impl<T: ?Sized> Deref for OwnedObject<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: inner is always initialized in [`Self::from_raw`] and
        // only taken in [`Drop::drop`].
        unsafe { self.inner.as_ref().unwrap_unchecked() }
    }
}

impl<T: ?Sized> Drop for OwnedObject<T> {
    fn drop(&mut self) {
        // SAFETY: inner is always initialized.
        let inner = unsafe { self.inner.take().unwrap_unchecked() };
        // Forget the Arc — leaves the outer strong count untouched.
        // Java retains its master `Arc` reference.
        std::mem::forget(inner);
    }
}

impl<T: ?Sized> OwnedObject<T> {
    /// Reconstruct an `Arc<T>` from a raw pointer obtained via
    /// [`Arc::into_raw`] and wrap it so the strong count is **not**
    /// decremented when the wrapper drops.
    ///
    /// # Safety
    ///
    /// `ptr` must be the result of an earlier `Arc::into_raw(Arc<T>)`
    /// and the strong count must still be > 0 (i.e. Java still owns it).
    pub unsafe fn from_raw(ptr: *const T) -> Self {
        Self {
            inner: Some(Arc::from_raw(ptr)),
        }
    }
}
