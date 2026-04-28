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

use std::sync::Arc;

use crate::{errors::ZResult, zerror};
use prebindgen_proc_macro::prebindgen;
use zenoh::key_expr::{KeyExpr as ZKeyExpr, SetIntersectionLevel};

/// Universal key-expression handle shared across the flat layer.
///
/// `string` is the validated text form of the key expression — kept on
/// the host (Java/Kotlin) side so cross-boundary calls that only inspect
/// the string don't need to enter Rust.
///
/// `ptr` is `Some` for session-declared key expressions: the `Arc`
/// holds zenoh's hidden registration id and must not be dropped while
/// the registration is alive. It is `None` for string-only expressions
/// built via [`try_from`] / [`autocanonize`].
///
/// Field is named `ptr` to mirror the JNI-side `JNIKeyExpr.ptr` it
/// round-trips through.
pub struct KeyExpr {
    pub string: String,
    pub ptr: Option<Arc<ZKeyExpr<'static>>>,
}

impl KeyExpr {
    /// Materialize a borrowed zenoh `KeyExpr` for one-shot zenoh calls.
    ///
    /// `Some(arc)` → cheap clone of the inner registered `KeyExpr`.
    /// `None` → unchecked-construct from the wrapper's already-validated
    /// string (the wrapper is only ever built via the validating
    /// constructors below, so the unchecked path is sound).
    pub(crate) fn as_zenoh(&self) -> ZKeyExpr<'static> {
        match &self.ptr {
            Some(arc) => (**arc).clone(),
            // SAFETY: every `KeyExpr` is built via the validating
            // constructors below (`try_from`, `autocanonize`, or by joining
            // / concatenating an already-validated wrapper), so
            // `self.string` is guaranteed to be a syntactically valid Zenoh
            // key expression.
            None => unsafe { ZKeyExpr::from_string_unchecked(self.string.clone()) },
        }
    }
}

/// Validate that `s` is a syntactically valid Zenoh key expression and
/// return it as a string-only [`KeyExpr`].
#[prebindgen]
pub fn try_from(s: String) -> ZResult<KeyExpr> {
    ZKeyExpr::try_from(s.as_str())
        .map_err(|err| zerror!("Unable to create key expression: '{}'.", err))?;
    Ok(KeyExpr {
        string: s,
        ptr: None,
    })
}

/// Auto-canonize `s` and return the canonized form as a string-only
/// [`KeyExpr`].
#[prebindgen]
pub fn autocanonize(s: String) -> ZResult<KeyExpr> {
    let canonized = ZKeyExpr::autocanonize(s)
        .map_err(|err| zerror!("Unable to create key expression: '{}'", err))?;
    Ok(KeyExpr {
        string: canonized.to_string(),
        ptr: None,
    })
}

/// True if `a` and `b` intersect.
#[prebindgen]
pub fn intersects(a: &KeyExpr, b: &KeyExpr) -> ZResult<bool> {
    Ok(a.as_zenoh().intersects(&b.as_zenoh()))
}

/// True if `a` includes `b`.
#[prebindgen]
pub fn includes(a: &KeyExpr, b: &KeyExpr) -> ZResult<bool> {
    Ok(a.as_zenoh().includes(&b.as_zenoh()))
}

/// Set-intersection level of `a` and `b` from `a`'s perspective.
/// Returns zenoh's [`SetIntersectionLevel`]; the JNI wrapper casts to
/// `i32` to match `io.zenoh.keyexpr.SetIntersectionLevel`
/// (0=DISJOINT, 1=INTERSECTS, 2=INCLUDES, 3=EQUALS).
#[prebindgen]
pub fn relation_to(a: &KeyExpr, b: &KeyExpr) -> ZResult<SetIntersectionLevel> {
    Ok(a.as_zenoh().relation_to(&b.as_zenoh()))
}

/// Join `a` with `other` using `/` and return the joined key expression.
/// Mirrors zenoh's `KeyExpr::join(&self, other: &str)`. When `a` is
/// session-declared the result also carries an `Arc` (preserving the
/// declaration handle).
#[prebindgen]
pub fn join(a: &KeyExpr, other: String) -> ZResult<KeyExpr> {
    let joined = a
        .as_zenoh()
        .join(other.as_str())
        .map_err(|err| zerror!(err))?;
    let string = joined.to_string();
    let ptr = if a.ptr.is_some() {
        Some(Arc::new(joined.into()))
    } else {
        None
    };
    Ok(KeyExpr { string, ptr })
}

/// Concatenate `a` with `other` (raw string concat) and return the result.
/// Mirrors zenoh's `KeyExpr::concat(&self, other: &str)`. Same handle-
/// preservation rule as [`join`].
#[prebindgen]
pub fn concat(a: &KeyExpr, other: String) -> ZResult<KeyExpr> {
    let concatenated = a
        .as_zenoh()
        .concat(other.as_str())
        .map_err(|err| zerror!(err))?;
    let string = concatenated.to_string();
    let ptr = if a.ptr.is_some() {
        Some(Arc::new(concatenated.into()))
    } else {
        None
    };
    Ok(KeyExpr { string, ptr })
}

/// Drop a [`KeyExpr`] handle obtained from a session-declared key
/// expression. Consuming the wrapper releases the `Arc<ZKeyExpr>` strong
/// reference (which the JNI side previously held). String-only
/// expressions (where `ptr` is `None`) are a no-op.
#[prebindgen]
pub fn drop_key_expr(key_expr: KeyExpr) -> ZResult<()> {
    drop(key_expr);
    Ok(())
}
