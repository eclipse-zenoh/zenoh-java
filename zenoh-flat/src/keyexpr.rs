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
/// `ptr` is `0` for string-only expressions built via [`try_from`] /
/// [`autocanonize`].  Non-zero values are `Arc<ZKeyExpr<'static>>` raw
/// pointers: the `Arc` holds zenoh's hidden registration id and must not
/// be dropped while the registration is alive.
///
/// Field order (`ptr` first) matches the `JNIKeyExpr(ptr: Long, string: String)`
/// Kotlin constructor so positional call sites need no update.
#[prebindgen_proc_macro::prebindgen]
#[derive(Debug, Clone)]
pub struct KeyExpr {
    pub ptr: i64,
    pub string: String,
}

impl KeyExpr {
    /// Materialize a borrowed zenoh `KeyExpr` for one-shot zenoh calls.
    ///
    /// When `ptr != 0`, bumps the Arc strong count, clones the inner
    /// `ZKeyExpr`, then drops the temporary Arc (net count unchanged).
    /// When `ptr == 0`, constructs from the already-validated string.
    pub(crate) fn as_zenoh(&self) -> ZKeyExpr<'static> {
        if self.ptr != 0 {
            let raw = self.ptr as *const ZKeyExpr<'static>;
            // SAFETY: `ptr` was produced by `Arc::into_raw`; we increment
            // before taking ownership so the Java-side strong ref survives.
            unsafe {
                Arc::increment_strong_count(raw);
                (*Arc::from_raw(raw)).clone()
            }
        } else {
            // SAFETY: every `KeyExpr` is built via the validating
            // constructors below (`try_from`, `autocanonize`, or by joining
            // / concatenating an already-validated wrapper), so
            // `self.string` is a syntactically valid Zenoh key expression.
            unsafe { ZKeyExpr::from_string_unchecked(self.string.clone()) }
        }
    }
}

/// Validate that `s` is a syntactically valid Zenoh key expression and
/// return it as a string-only [`KeyExpr`].
#[prebindgen]
pub fn try_from(s: String) -> ZResult<KeyExpr> {
    ZKeyExpr::try_from(s.as_str())
        .map_err(|err| zerror!("Unable to create key expression: '{}'.", err))?;
    Ok(KeyExpr { ptr: 0, string: s })
}

/// Auto-canonize `s` and return the canonized form as a string-only
/// [`KeyExpr`].
#[prebindgen]
pub fn autocanonize(s: String) -> ZResult<KeyExpr> {
    let canonized = ZKeyExpr::autocanonize(s)
        .map_err(|err| zerror!("Unable to create key expression: '{}'", err))?;
    Ok(KeyExpr {
        ptr: 0,
        string: canonized.to_string(),
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
/// session-declared the result also carries a fresh Arc (new registration).
#[prebindgen]
pub fn join(a: &KeyExpr, other: String) -> ZResult<KeyExpr> {
    let joined = a
        .as_zenoh()
        .join(other.as_str())
        .map_err(|err| zerror!(err))?;
    let string = joined.to_string();
    let ptr = if a.ptr != 0 {
        // SAFETY: `Arc::into_raw` transfers ownership to the Java caller.
        let arc: Arc<ZKeyExpr<'static>> = Arc::new(joined.into());
        Arc::into_raw(arc) as i64
    } else {
        0
    };
    Ok(KeyExpr { ptr, string })
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
    let ptr = if a.ptr != 0 {
        let arc: Arc<ZKeyExpr<'static>> = Arc::new(concatenated.into());
        Arc::into_raw(arc) as i64
    } else {
        0
    };
    Ok(KeyExpr { ptr, string })
}

/// Drop a [`KeyExpr`] handle obtained from a session-declared key
/// expression. When `ptr != 0`, takes ownership of the `Arc<ZKeyExpr>`
/// (decrementing the strong count). String-only expressions are a no-op.
#[prebindgen]
pub fn drop_key_expr(key_expr: KeyExpr) -> ZResult<()> {
    if key_expr.ptr != 0 {
        // SAFETY: `ptr` was produced by `Arc::into_raw`; taking ownership
        // here and letting the Arc drop releases the Java-side strong ref.
        unsafe { drop(Arc::from_raw(key_expr.ptr as *const ZKeyExpr<'static>)) };
    }
    Ok(())
}
