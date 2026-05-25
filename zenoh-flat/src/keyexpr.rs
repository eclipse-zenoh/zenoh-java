use crate::Error;
use crate::ZKeyExpr;
use crate::z_keyexpr_autocanonize;
use crate::z_keyexpr_intersects;
use prebindgen_proc_macro::prebindgen;

#[prebindgen]
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
    Ok(a_ke.includes(&b_ke))
}

/// Returns the integer discriminant of `zenoh::key_expr::SetIntersectionLevel`
/// describing the relation of `a` to `b`.
#[prebindgen]
pub fn keyexpr_relation_to(
    a: impl Into<KeyExpr> + Send + 'static,
    b: impl Into<KeyExpr> + Send + 'static,
) -> Result<i32, Error> {
    let a_ke = into_native(a.into())?;
    let b_ke = into_native(b.into())?;
    Ok(a_ke.relation_to(&b_ke) as i32)
}

/// Joins `a` and `b` with a `/` separator and returns the resulting KeyExpr.
#[prebindgen]
pub fn keyexpr_join(
    a: impl Into<KeyExpr> + Send + 'static,
    b: String,
) -> Result<KeyExpr, Error> {
    let a_ke = into_native(a.into())?;
    let joined = a_ke.join(&b)?;
    Ok(KeyExpr::from(joined))
}

/// Concatenates `b` onto `a` and returns the resulting KeyExpr if valid.
#[prebindgen]
pub fn keyexpr_concat(
    a: impl Into<KeyExpr> + Send + 'static,
    b: String,
) -> Result<KeyExpr, Error> {
    let a_ke = into_native(a.into())?;
    let concatenated = a_ke.concat(&b)?;
    Ok(KeyExpr::from(concatenated))
}

fn into_native(k: KeyExpr) -> Result<ZKeyExpr, Error> {
    match k.key_expr_native {
        Some(ke) => Ok(ke),
        None => Ok(ZKeyExpr::try_from(k.key_expr_string)?),
    }
}
