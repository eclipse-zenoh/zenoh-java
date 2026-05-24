use prebindgen_proc_macro::prebindgen;
use crate::ZKeyExpr;
use crate::Error;

// #[prebindgen]
pub fn z_keyexpr_try_from(s: String) -> Result<ZKeyExpr, Error> {
    let ke = ZKeyExpr::try_from(s)?;
    Ok(ke)
}

// #[prebindgen]
pub fn z_keyexpr_autocanonize(s: String) -> Result<ZKeyExpr, Error> {
    let ke = ZKeyExpr::autocanonize(s)?;
    Ok(ke)
}
