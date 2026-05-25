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
    let ke = ZKeyExpr::try_from(s.clone())?;
    Ok(KeyExpr {
        key_expr_string: s,
        key_expr_native: Some(ke),
    })
}

/// Convert a key expression string into it's canonical form
#[prebindgen]
pub fn keyexpr_autocanonize(s: String) -> Result<KeyExpr, Error> {
    let ke = z_keyexpr_autocanonize(s.clone())?;
    Ok(KeyExpr {
        key_expr_string: s,
        key_expr_native: Some(ke),
    })
}

/// Returns true if keyexpr a and b intersect, false otherwise
#[prebindgen]
pub fn keyexpr_intersects(
    a: impl Into<KeyExpr> + Send + 'static,
    b: impl Into<KeyExpr> + Send + 'static,
) -> Result<bool, Error> {
    let a = a.into();
    let b = b.into();
    let a_ke = match a.key_expr_native {
        Some(ke) => ke,
        None => ZKeyExpr::try_from(a.key_expr_string)?,
    };
    let b_ke = match b.key_expr_native {
        Some(ke) => ke,
        None => ZKeyExpr::try_from(b.key_expr_string)?,
    };
    Ok(z_keyexpr_intersects(&a_ke, &b_ke))
}
