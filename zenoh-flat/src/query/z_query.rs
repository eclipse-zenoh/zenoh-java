use crate::{into_native, Encoding, Error, KeyExpr, ZBytes, ZEncoding, ZQuery};
use prebindgen_proc_macro::prebindgen;
use zenoh::{
    time::{Timestamp, TimestampId, NTP64},
    Wait,
};

#[prebindgen]
pub fn z_query_reply_success(
    query: ZQuery,
    key_expr: impl Into<KeyExpr> + Send + 'static,
    payload: impl Into<ZBytes> + Send + 'static,
    encoding: impl Into<Encoding> + Send + 'static,
    timestamp_ntp64: Option<i64>,
    attachment: Option<impl Into<ZBytes> + Send + 'static>,
    express: bool,
) -> Result<(), Error> {
    let ke = into_native(key_expr.into())?;
    let payload: ZBytes = payload.into();
    let z_encoding: ZEncoding = encoding.into().try_into()?;
    let mut b = query.reply(ke, payload.bytes).encoding(z_encoding);
    if let Some(ntp) = timestamp_ntp64 {
        b = b.timestamp(Timestamp::new(NTP64(ntp as u64), TimestampId::rand()));
    }
    if let Some(att) = attachment {
        let att: ZBytes = att.into();
        b = b.attachment::<Vec<u8>>(att.bytes);
    }
    b.express(express).wait().map_err(Error::from)
}

#[prebindgen]
pub fn z_query_reply_error(
    query: ZQuery,
    payload: impl Into<ZBytes> + Send + 'static,
    encoding: impl Into<Encoding> + Send + 'static,
) -> Result<(), Error> {
    let payload: ZBytes = payload.into();
    let z_encoding: ZEncoding = encoding.into().try_into()?;
    query
        .reply_err(payload.bytes)
        .encoding(z_encoding)
        .wait()
        .map_err(Error::from)
}

#[prebindgen]
pub fn z_query_reply_delete(
    query: ZQuery,
    key_expr: impl Into<KeyExpr> + Send + 'static,
    timestamp_ntp64: Option<i64>,
    attachment: Option<impl Into<ZBytes> + Send + 'static>,
    express: bool,
) -> Result<(), Error> {
    let ke = into_native(key_expr.into())?;
    let mut b = query.reply_del(ke);
    if let Some(ntp) = timestamp_ntp64 {
        b = b.timestamp(Timestamp::new(NTP64(ntp as u64), TimestampId::rand()));
    }
    if let Some(att) = attachment {
        let att: ZBytes = att.into();
        b = b.attachment::<Vec<u8>>(att.bytes);
    }
    b.express(express).wait().map_err(Error::from)
}
