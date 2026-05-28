use crate::{Error, ZBytes, ZPublisher};
use prebindgen_proc_macro::prebindgen;
use zenoh::{bytes::Encoding, Wait};

#[prebindgen]
pub fn z_publisher_put(
    publisher: &ZPublisher,
    payload: impl Into<ZBytes> + Send + 'static,
    encoding_id: i32,
    encoding_schema: Option<String>,
    attachment: Option<impl Into<ZBytes> + Send + 'static>,
) -> Result<(), Error> {
    let id = u16::try_from(encoding_id).map_err(|e| Error {
        message: format!("Invalid encoding id {encoding_id}: {e}"),
    })?;
    let schema = encoding_schema.map(|s| s.into_bytes().into());
    let encoding = Encoding::new(id, schema);
    let payload: ZBytes = payload.into();
    let mut publication = publisher.put(payload.bytes).encoding(encoding);
    if let Some(att) = attachment {
        let att: ZBytes = att.into();
        publication = publication.attachment::<Vec<u8>>(att.bytes);
    }
    publication.wait().map_err(Error::from)
}

#[prebindgen]
pub fn z_publisher_delete(
    publisher: &ZPublisher,
    attachment: Option<impl Into<ZBytes> + Send + 'static>,
) -> Result<(), Error> {
    let mut delete = publisher.delete();
    if let Some(att) = attachment {
        let att: ZBytes = att.into();
        delete = delete.attachment::<Vec<u8>>(att.bytes);
    }
    delete.wait().map_err(Error::from)
}
