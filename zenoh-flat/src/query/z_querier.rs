use super::reply::Reply;
use crate::util::OnceDrop;
use crate::{Encoding, Error, ZBytes, ZEncoding, ZQuerier, ZReply};
use prebindgen_proc_macro::prebindgen;
use zenoh::Wait;

/// Perform a GET through a querier, delivering each reply as an opaque
/// [`ZReply`] handle (thin surface — cheap-FFI bindings pull fields via the
/// `z_reply_*` accessors). `on_close` fires when the reply stream ends.
#[prebindgen]
pub fn z_querier_get(
    querier: &ZQuerier,
    parameters: Option<String>,
    payload: Option<impl Into<ZBytes> + Send + 'static>,
    encoding: impl Into<Encoding> + Send + 'static,
    attachment: Option<impl Into<ZBytes> + Send + 'static>,
    callback: impl Fn(ZReply) + Send + Sync + 'static,
    on_close: impl Fn() + Send + Sync + 'static,
) -> Result<(), Error> {
    let on_close = OnceDrop::new(on_close);
    let mut builder = querier.get();
    if let Some(params) = parameters {
        builder = builder.parameters(params);
    }
    if let Some(payload) = payload {
        let payload: ZBytes = payload.into();
        let encoding: ZEncoding = encoding.into().try_into()?;
        builder = builder.payload(payload.bytes).encoding(encoding);
    }
    if let Some(attachment) = attachment {
        let attachment: ZBytes = attachment.into();
        builder = builder.attachment::<Vec<u8>>(attachment.bytes);
    }
    builder
        .callback(move |reply| {
            let _ = &on_close;
            callback(reply);
        })
        .wait()
        .map_err(Error::from)
}

/// Perform a GET through a querier, delivering each reply as a fully decoded
/// [`Reply`] data class (thick surface — one FFI hop per reply for
/// expensive-FFI bindings). See [`z_querier_get`] for parameter semantics.
#[prebindgen]
pub fn querier_get(
    querier: &ZQuerier,
    parameters: Option<String>,
    payload: Option<impl Into<ZBytes> + Send + 'static>,
    encoding: impl Into<Encoding> + Send + 'static,
    attachment: Option<impl Into<ZBytes> + Send + 'static>,
    callback: impl Fn(Reply) + Send + Sync + 'static,
    on_close: impl Fn() + Send + Sync + 'static,
) -> Result<(), Error> {
    z_querier_get(
        querier,
        parameters,
        payload,
        encoding,
        attachment,
        move |zr| callback(Reply::from(&zr)),
        on_close,
    )
}
