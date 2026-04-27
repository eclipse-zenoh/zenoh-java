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
use zenoh::key_expr::KeyExpr;

/// Newtype marker for "owning take of a declared `KeyExpr` handle by raw
/// pointer". The JNI wrapper layer (`zenoh-jni/build.rs`) registers the wire
/// form as `*const KeyExpr<'static>` and supplies a decoder that reconstructs
/// the `Arc`, clones the inner `KeyExpr`, and drops the temporary `Arc`
/// (releasing the JNI strong reference). Used by [`drop_key_expr`] to
/// disambiguate from the `KeyExpr<'static>` ↔ `JObject` (declared+undeclared
/// holder) wire used by all other key-expression operations.
pub struct OwnedKeyExpr(pub KeyExpr<'static>);

/// Validate that `s` is a syntactically valid Zenoh key expression and
/// return it back unchanged on success.
#[prebindgen_proc_macro::prebindgen]
pub fn try_from(s: String) -> ZResult<String> {
    KeyExpr::try_from(s.clone())
        .map(|_| s)
        .map_err(|err| zerror!("Unable to create key expression: '{}'.", err))
}

/// Auto-canonize `s` and return the canonized form.
#[prebindgen_proc_macro::prebindgen]
pub fn autocanonize(s: String) -> ZResult<String> {
    KeyExpr::autocanonize(s)
        .map(|k| k.to_string())
        .map_err(|err| zerror!("Unable to create key expression: '{}'", err))
}

/// True iff `a` and `b` intersect.
#[prebindgen_proc_macro::prebindgen]
pub fn intersects(a: KeyExpr<'static>, b: KeyExpr<'static>) -> ZResult<bool> {
    Ok(a.intersects(&b))
}

/// True iff `a` includes `b`.
#[prebindgen_proc_macro::prebindgen]
pub fn includes(a: KeyExpr<'static>, b: KeyExpr<'static>) -> ZResult<bool> {
    Ok(a.includes(&b))
}

/// Set-intersection level of `a` and `b` from `a`'s perspective, encoded as
/// the integer matching `io.zenoh.keyexpr.SetIntersectionLevel` (0=DISJOINT,
/// 1=INTERSECTS, 2=INCLUDES, 3=EQUALS).
#[prebindgen_proc_macro::prebindgen]
pub fn relation_to(a: KeyExpr<'static>, b: KeyExpr<'static>) -> ZResult<i32> {
    Ok(a.relation_to(&b) as i32)
}

/// Join `a` with `other` using `/` and return the joined key expression.
#[prebindgen_proc_macro::prebindgen]
pub fn join(a: KeyExpr<'static>, other: String) -> ZResult<String> {
    a.join(other.as_str())
        .map(|k| k.to_string())
        .map_err(|err| zerror!(err))
}

/// Concatenate `a` with `other` (raw string concat) and return the result.
#[prebindgen_proc_macro::prebindgen]
pub fn concat(a: KeyExpr<'static>, other: String) -> ZResult<String> {
    a.concat(other.as_str())
        .map(|k| k.to_string())
        .map_err(|err| zerror!(err))
}

/// Drop an [`OwnedKeyExpr`] handle obtained from a session-declared key
/// expression. The JNI wrapper reconstructs the `Arc<KeyExpr>` from the raw
/// pointer; this fn just consumes the value so the temporary `Arc` is
/// released at end of scope.
#[prebindgen_proc_macro::prebindgen]
pub fn drop_key_expr(key_expr: OwnedKeyExpr) -> ZResult<()> {
    drop(key_expr);
    Ok(())
}
