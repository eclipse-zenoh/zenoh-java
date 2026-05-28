use crate::Error;
use crate::ZKeyExpr;
use super::z_keyexpr::z_keyexpr_autocanonize;
use super::z_keyexpr::z_keyexpr_concat;
use super::z_keyexpr::z_keyexpr_includes;
use super::z_keyexpr::z_keyexpr_intersects;
use super::z_keyexpr::z_keyexpr_join;
use super::z_keyexpr::z_keyexpr_relation_to;
use prebindgen_proc_macro::prebindgen;

/// Mirrors `zenoh::key_expr::SetIntersectionLevel` with a stable FFI surface.
#[prebindgen]
pub enum SetIntersectionLevel {
    Disjoint = 0,
    Intersects = 1,
    Includes = 2,
    Equals = 3,
}

impl From<zenoh::key_expr::SetIntersectionLevel> for SetIntersectionLevel {
    fn from(value: zenoh::key_expr::SetIntersectionLevel) -> Self {
        match value {
            zenoh::key_expr::SetIntersectionLevel::Disjoint => SetIntersectionLevel::Disjoint,
            zenoh::key_expr::SetIntersectionLevel::Intersects => SetIntersectionLevel::Intersects,
            zenoh::key_expr::SetIntersectionLevel::Includes => SetIntersectionLevel::Includes,
            zenoh::key_expr::SetIntersectionLevel::Equals => SetIntersectionLevel::Equals,
        }
    }
}

#[prebindgen]
#[derive(Clone)]
pub struct KeyExpr {
    pub key_expr_string: String,
    pub key_expr_native: Option<ZKeyExpr>,
}

impl From<String> for KeyExpr {
    fn from(s: String) -> Self {
        KeyExpr {
            key_expr_string: s,
            key_expr_native: None,
        }
    }
}

impl From<&str> for KeyExpr {
    fn from(s: &str) -> Self {
        KeyExpr {
            key_expr_string: s.to_string(),
            key_expr_native: None,
        }
    }
}

impl From<ZKeyExpr> for KeyExpr {
    fn from(ke: ZKeyExpr) -> Self {
        KeyExpr {
            key_expr_string: ke.to_string(),
            key_expr_native: Some(ke),
        }
    }
}

/// Validate that string `s` is a syntactically valid Zenoh key expression
#[prebindgen]
pub fn keyexpr_try_from(s: String) -> Result<KeyExpr, Error> {
    let ke = ZKeyExpr::try_from(s)?;
    Ok(KeyExpr::from(ke))
}

/// Convert a key expression string into it's canonical form
#[prebindgen]
pub fn keyexpr_autocanonize(s: String) -> Result<KeyExpr, Error> {
    let ke = z_keyexpr_autocanonize(s)?;
    Ok(KeyExpr::from(ke))
}

/// Returns true if keyexpr a and b intersect, false otherwise
#[prebindgen]
pub fn keyexpr_intersects(
    a: impl Into<KeyExpr> + Send + 'static,
    b: impl Into<KeyExpr> + Send + 'static,
) -> Result<bool, Error> {
    let a_ke = into_native(a.into())?;
    let b_ke = into_native(b.into())?;
    Ok(z_keyexpr_intersects(&a_ke, &b_ke))
}

/// Returns true if keyexpr a includes keyexpr b (every key matched by b is matched by a).
#[prebindgen]
pub fn keyexpr_includes(
    a: impl Into<KeyExpr> + Send + 'static,
    b: impl Into<KeyExpr> + Send + 'static,
) -> Result<bool, Error> {
    let a_ke = into_native(a.into())?;
    let b_ke = into_native(b.into())?;
    Ok(z_keyexpr_includes(&a_ke, &b_ke))
}

/// Returns `SetIntersectionLevel` describing the relation of `a` to `b`.
#[prebindgen]
pub fn keyexpr_relation_to(
    a: impl Into<KeyExpr> + Send + 'static,
    b: impl Into<KeyExpr> + Send + 'static,
) -> Result<SetIntersectionLevel, Error> {
    let a_ke = into_native(a.into())?;
    let b_ke = into_native(b.into())?;
    Ok(z_keyexpr_relation_to(&a_ke, &b_ke))
}

/// Joins `a` and `b` with a `/` separator and returns the resulting KeyExpr.
#[prebindgen]
pub fn keyexpr_join(
    a: impl Into<KeyExpr> + Send + 'static,
    b: String,
) -> Result<KeyExpr, Error> {
    let a_ke = into_native(a.into())?;
    Ok(KeyExpr::from(z_keyexpr_join(&a_ke, b)?))
}

/// Concatenates `b` onto `a` and returns the resulting KeyExpr if valid.
#[prebindgen]
pub fn keyexpr_concat(
    a: impl Into<KeyExpr> + Send + 'static,
    b: String,
) -> Result<KeyExpr, Error> {
    let a_ke = into_native(a.into())?;
    Ok(KeyExpr::from(z_keyexpr_concat(&a_ke, b)?))
}

pub(crate) fn into_native(k: KeyExpr) -> Result<ZKeyExpr, Error> {
    match k.key_expr_native {
        Some(ke) => Ok(ke),
        None => Ok(ZKeyExpr::try_from(k.key_expr_string)?),
    }
}
