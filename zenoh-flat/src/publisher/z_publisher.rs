use crate::{Encoding, Error, ZBytes, ZEncoding, ZPublisher};
use prebindgen_proc_macro::prebindgen;
use zenoh::Wait;

#[prebindgen]
pub fn z_publisher_put(
    publisher: &ZPublisher,
    payload: impl Into<ZBytes> + Send + 'static,
    encoding: impl Into<Encoding> + Send + 'static,
    attachment: Option<impl Into<ZBytes> + Send + 'static>,
) -> Result<(), Error> {
    let encoding: Encoding = encoding.into();
    let z_encoding: ZEncoding = encoding.try_into()?;
    let payload: ZBytes = payload.into();
    let mut publication = publisher.put(payload.bytes).encoding(z_encoding);
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
