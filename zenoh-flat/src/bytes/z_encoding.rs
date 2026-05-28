use crate::ZEncoding;
use prebindgen_proc_macro::prebindgen;

/// Numeric id of the encoding (u16 widened to i32 for JVM).
#[prebindgen]
pub fn z_encoding_id(e: &ZEncoding) -> i32 {
    e.id() as i32
}

/// Optional textual schema attached to the encoding.
#[prebindgen]
pub fn z_encoding_schema(e: &ZEncoding) -> Option<String> {
    e.schema()
        .and_then(|s| std::str::from_utf8(s).ok().map(str::to_string))
}
