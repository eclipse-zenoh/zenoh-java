use crate::{Error, ZBytes, ZPublisher};
use prebindgen_proc_macro::prebindgen;
use zenoh::{bytes::Encoding, Wait};

#[prebindgen]
pub fn z_publisher_put(
    publisher: &ZPublisher,
    payload: impl Into<ZBytes> + Send + 'static,
    encoding_id: i32,
    encoding_schema: Option<String>,
    attachment: Option<Vec<u8>>,
) -> Result<(), Error> {
    let id = u16::try_from(encoding_id).map_err(|e| Error {
        message: format!("Invalid encoding id {encoding_id}: {e}"),
    })?;
    let schema = encoding_schema.map(|s| s.into_bytes().into());
    let encoding = Encoding::new(id, schema);
    let payload: ZBytes = payload.into();
    let mut publication = publisher.put(payload.bytes).encoding(encoding);
    if let Some(att) = attachment {
        publication = publication.attachment::<Vec<u8>>(att);
    }
    publication.wait().map_err(Error::from)
}

#[prebindgen]
pub fn z_publisher_delete(
    publisher: &ZPublisher,
    attachment: Option<Vec<u8>>,
) -> Result<(), Error> {
    let mut delete = publisher.delete();
    if let Some(att) = attachment {
        delete = delete.attachment::<Vec<u8>>(att);
    }
    delete.wait().map_err(Error::from)
}
