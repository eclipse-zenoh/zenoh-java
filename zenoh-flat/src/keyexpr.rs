use prebindgen_proc_macro::prebindgen;
use crate::ZKeyExpr;
use crate::Error;

/// Validate that `s` is a syntactically valid Zenoh key expression
#[prebindgen]
pub fn keyexpr_validate(s: String) -> Result<String, Error> {
    ZKeyExpr::try_from(s.as_str())?;
    Ok(s)
}
