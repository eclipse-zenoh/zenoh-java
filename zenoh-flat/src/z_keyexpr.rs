use prebindgen_proc_macro::prebindgen;
use crate::ZKeyExpr;
use crate::Error;

#[prebindgen]
pub fn z_keyexpr_try_from(s: String) -> Result<ZKeyExpr, Error> {
    let ke = ZKeyExpr::try_from(s)?;
    Ok(ke)
}

#[prebindgen]
pub fn z_keyexpr_autocanonize(s: String) -> Result<ZKeyExpr, Error> {
    let ke = ZKeyExpr::autocanonize(s)?;
    Ok(ke)
}

#[prebindgen]
pub fn z_keyexpr_intersects(a: &ZKeyExpr, b: &ZKeyExpr) -> bool {
    a.intersects(b)
}

#[prebindgen]
pub fn z_keyexpr_includes(a: &ZKeyExpr, b: &ZKeyExpr) -> bool {
    a.includes(b)
}

#[prebindgen]
pub fn z_keyexpr_relation_to(a: &ZKeyExpr, b: &ZKeyExpr) -> i32 {
    a.relation_to(b) as i32
}

#[prebindgen]
pub fn z_keyexpr_join(a: &ZKeyExpr, b: String) -> Result<ZKeyExpr, Error> {
    Ok(a.join(&b)?)
}

#[prebindgen]
pub fn z_keyexpr_concat(a: &ZKeyExpr, b: String) -> Result<ZKeyExpr, Error> {
    Ok(a.concat(&b)?)
}
