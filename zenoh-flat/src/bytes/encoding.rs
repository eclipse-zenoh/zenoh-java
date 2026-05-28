use super::z_encoding::{z_encoding_id, z_encoding_schema};
use crate::{Error, ZEncoding};
use prebindgen_proc_macro::prebindgen;
use zenoh::internal::buffers::ZSlice;

/// Value-class twin of [`ZEncoding`]. Carries decomposed fields so
/// bindings with expensive FFI boundaries can read id/schema without
/// round-tripping into native code.
#[prebindgen]
#[derive(Clone)]
pub struct Encoding {
    pub id: i32,
    pub schema: Option<String>,
}

impl From<&ZEncoding> for Encoding {
    fn from(e: &ZEncoding) -> Self {
        Self {
            id: z_encoding_id(e),
            schema: z_encoding_schema(e),
        }
    }
}

impl From<ZEncoding> for Encoding {
    fn from(e: ZEncoding) -> Self {
        Self::from(&e)
    }
}

impl TryFrom<Encoding> for ZEncoding {
    type Error = Error;
    fn try_from(e: Encoding) -> Result<Self, Error> {
        let id = u16::try_from(e.id).map_err(|err| Error {
            message: format!("Invalid encoding id {}: {err}", e.id),
        })?;
        let schema = e.schema.map(|s| ZSlice::from(s.into_bytes()));
        Ok(ZEncoding::new(id, schema))
    }
}

impl TryFrom<&Encoding> for ZEncoding {
    type Error = Error;
    fn try_from(e: &Encoding) -> Result<Self, Error> {
        ZEncoding::try_from(e.clone())
    }
}
