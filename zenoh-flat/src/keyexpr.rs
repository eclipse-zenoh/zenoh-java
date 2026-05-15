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

//! Key-expression operations exposed to the JNI / Kotlin layer.
//!
//! No wrapper struct: pure validators (`try_from`, `autocanonize`) return a
//! `String`; constructors that produce a registered handle (`join`, `concat`,
//! see also `session::declare_key_expr`) return zenoh's native
//! [`KeyExpr<'static>`] treated as an opaque Arc handle.
//!
//! Functions that consume a key-expression accept
//! `impl Into<KeyExpr<'static>> + Send + 'static`. The JNI plugin provides
//! the input converter that decodes the Java `KeyExpr(ptr, string)` data
//! class — non-zero `ptr` clones the Arc, otherwise the string is
//! validated and converted to an owned key expression.

use crate::{errors::ZResult, zerror};
use prebindgen_proc_macro::prebindgen;
use zenoh::key_expr::{KeyExpr as ZKeyExpr, SetIntersectionLevel};

/// Validate that `s` is a syntactically valid Zenoh key expression and
/// return it (unchanged). The Kotlin side wraps the result as an
/// undeclared `KeyExpr(ptr=0, string=s)`.
#[prebindgen]
pub fn try_from(s: String) -> ZResult<String> {
    ZKeyExpr::try_from(s.as_str())
        .map_err(|err| zerror!("Unable to create key expression: '{}'.", err))?;
    Ok(s)
}

/// Auto-canonize `s` and return the canonized string form.
#[prebindgen]
pub fn autocanonize(s: String) -> ZResult<String> {
    ZKeyExpr::autocanonize(s.clone())
        .map(|ke| ke.to_string())
        .map_err(|err| zerror!("Unable to create key expression: '{}'", err))
}

/// True if `a` and `b` intersect.
#[prebindgen]
pub fn intersects(
    a: impl Into<ZKeyExpr<'static>> + Send + 'static,
    b: impl Into<ZKeyExpr<'static>> + Send + 'static,
) -> ZResult<bool> {
    let a = a.into();
    let b = b.into();
    Ok(a.intersects(&b))
}

/// True if `a` includes `b`.
#[prebindgen]
pub fn includes(
    a: impl Into<ZKeyExpr<'static>> + Send + 'static,
    b: impl Into<ZKeyExpr<'static>> + Send + 'static,
) -> ZResult<bool> {
    let a = a.into();
    let b = b.into();
    Ok(a.includes(&b))
}

/// Set-intersection level of `a` and `b` from `a`'s perspective.
#[prebindgen]
pub fn relation_to(
    a: impl Into<ZKeyExpr<'static>> + Send + 'static,
    b: impl Into<ZKeyExpr<'static>> + Send + 'static,
) -> ZResult<SetIntersectionLevel> {
    let a = a.into();
    let b = b.into();
    Ok(a.relation_to(&b))
}

/// Join `a` with `other` using `/` and return the resulting key
/// expression. The result inherits `a`'s declaration registration when
/// present; either way Kotlin sees an Arc-backed handle.
#[prebindgen]
pub fn join(
    a: impl Into<ZKeyExpr<'static>> + Send + 'static,
    other: String,
) -> ZResult<ZKeyExpr<'static>> {
    let a = a.into();
    a.join(other.as_str()).map_err(|err| zerror!(err))
}

/// Concatenate `a` with `other` (raw string concat) and return the
/// result. Same handle-preservation rule as [`join`].
#[prebindgen]
pub fn concat(
    a: impl Into<ZKeyExpr<'static>> + Send + 'static,
    other: String,
) -> ZResult<ZKeyExpr<'static>> {
    let a = a.into();
    a.concat(other.as_str()).map_err(|err| zerror!(err))
}
