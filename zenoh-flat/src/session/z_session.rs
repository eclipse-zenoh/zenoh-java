use crate::util::OnceDrop;
use crate::{
    into_native, CongestionControl, ConsolidationMode, Encoding, Error, KeyExpr, Priority, Query,
    QueryTarget, Reliability, Reply, ReplyKeyExpr, Sample, ZBytes, ZConfig, ZEncoding, ZKeyExpr,
    ZPublisher, ZQuerier, ZQuery, ZQueryable, ZReply, ZSample, ZSession, ZSubscriber, ZenohId,
};
use prebindgen_proc_macro::prebindgen;
use std::time::Duration;
use zenoh::{query::Selector, Wait};

/// Open a session with the given configuration. The config is cloned, not
/// consumed, so the caller's handle stays valid (matching legacy behavior).
#[prebindgen]
pub fn z_open(config: &ZConfig) -> Result<ZSession, Error> {
    zenoh::open(config.clone()).wait().map_err(Error::from)
}

#[prebindgen]
pub fn z_session_declare_publisher(
    session: &ZSession,
    key_expr: impl Into<KeyExpr> + Send + 'static,
    congestion_control: CongestionControl,
    priority: Priority,
    express: bool,
    reliability: Reliability,
) -> Result<ZPublisher, Error> {
    let ke = into_native(key_expr.into())?;
    session
        .declare_publisher(ke)
        .congestion_control(congestion_control.into())
        .priority(priority.into())
        .express(express)
        .reliability(reliability.into())
        .wait()
        .map_err(Error::from)
}

#[prebindgen]
pub fn z_session_put(
    session: &ZSession,
    key_expr: impl Into<KeyExpr> + Send + 'static,
    payload: impl Into<ZBytes> + Send + 'static,
    encoding: impl Into<Encoding> + Send + 'static,
    congestion_control: CongestionControl,
    priority: Priority,
    express: bool,
    attachment: Option<impl Into<ZBytes> + Send + 'static>,
    reliability: Reliability,
) -> Result<(), Error> {
    let ke = into_native(key_expr.into())?;
    let payload: ZBytes = payload.into();
    let z_encoding: ZEncoding = encoding.into().try_into()?;
    let mut builder = session
        .put(&ke, payload.bytes)
        .congestion_control(congestion_control.into())
        .encoding(z_encoding)
        .express(express)
        .priority(priority.into())
        .reliability(reliability.into());
    if let Some(att) = attachment {
        let att: ZBytes = att.into();
        builder = builder.attachment::<Vec<u8>>(att.bytes);
    }
    builder.wait().map_err(Error::from)
}

#[prebindgen]
pub fn z_session_delete(
    session: &ZSession,
    key_expr: impl Into<KeyExpr> + Send + 'static,
    congestion_control: CongestionControl,
    priority: Priority,
    express: bool,
    attachment: Option<impl Into<ZBytes> + Send + 'static>,
    reliability: Reliability,
) -> Result<(), Error> {
    let ke = into_native(key_expr.into())?;
    let mut builder = session
        .delete(&ke)
        .congestion_control(congestion_control.into())
        .express(express)
        .priority(priority.into())
        .reliability(reliability.into());
    if let Some(att) = attachment {
        let att: ZBytes = att.into();
        builder = builder.attachment::<Vec<u8>>(att.bytes);
    }
    builder.wait().map_err(Error::from)
}

/// Declare a subscriber delivering each change as an opaque [`ZSample`] handle
/// (thin surface). `on_close` fires when the subscriber is dropped.
#[prebindgen]
pub fn z_session_declare_subscriber(
    session: &ZSession,
    key_expr: impl Into<KeyExpr> + Send + 'static,
    callback: impl Fn(ZSample) + Send + Sync + 'static,
    on_close: impl Fn() + Send + Sync + 'static,
) -> Result<ZSubscriber, Error> {
    let ke = into_native(key_expr.into())?;
    let on_close = OnceDrop::new(on_close);
    session
        .declare_subscriber(ke)
        .callback(move |sample| {
            let _ = &on_close;
            callback(sample);
        })
        .wait()
        .map_err(Error::from)
}

/// Declare a subscriber delivering each change as a fully decoded [`Sample`]
/// data class (thick surface). See [`z_session_declare_subscriber`].
#[prebindgen]
pub fn session_declare_subscriber(
    session: &ZSession,
    key_expr: impl Into<KeyExpr> + Send + 'static,
    callback: impl Fn(Sample) + Send + Sync + 'static,
    on_close: impl Fn() + Send + Sync + 'static,
) -> Result<ZSubscriber, Error> {
    z_session_declare_subscriber(
        session,
        key_expr,
        move |zs| callback(Sample::from(&zs)),
        on_close,
    )
}

#[prebindgen]
pub fn z_session_declare_querier(
    session: &ZSession,
    key_expr: impl Into<KeyExpr> + Send + 'static,
    target: QueryTarget,
    consolidation: ConsolidationMode,
    congestion_control: CongestionControl,
    priority: Priority,
    express: bool,
    timeout_ms: i64,
    accept_replies: ReplyKeyExpr,
) -> Result<ZQuerier, Error> {
    let ke = into_native(key_expr.into())?;
    let consolidation: zenoh::query::ConsolidationMode = consolidation.into();
    session
        .declare_querier(ke)
        .congestion_control(congestion_control.into())
        .consolidation(consolidation)
        .express(express)
        .target(target.into())
        .priority(priority.into())
        .timeout(Duration::from_millis(timeout_ms as u64))
        .accept_replies(accept_replies.into())
        .wait()
        .map_err(Error::from)
}

/// Declare a queryable delivering each query as an opaque [`ZQuery`] handle
/// (thin surface). `on_close` fires when the queryable is dropped.
#[prebindgen]
pub fn z_session_declare_queryable(
    session: &ZSession,
    key_expr: impl Into<KeyExpr> + Send + 'static,
    complete: bool,
    callback: impl Fn(ZQuery) + Send + Sync + 'static,
    on_close: impl Fn() + Send + Sync + 'static,
) -> Result<ZQueryable, Error> {
    let ke = into_native(key_expr.into())?;
    let on_close = OnceDrop::new(on_close);
    session
        .declare_queryable(ke)
        .complete(complete)
        .callback(move |query| {
            let _ = &on_close;
            callback(query);
        })
        .wait()
        .map_err(Error::from)
}

/// Declare a queryable delivering each query as a fully decoded [`Query`] data
/// class (thick surface). See [`z_session_declare_queryable`].
#[prebindgen]
pub fn session_declare_queryable(
    session: &ZSession,
    key_expr: impl Into<KeyExpr> + Send + 'static,
    complete: bool,
    callback: impl Fn(Query) + Send + Sync + 'static,
    on_close: impl Fn() + Send + Sync + 'static,
) -> Result<ZQueryable, Error> {
    z_session_declare_queryable(
        session,
        key_expr,
        complete,
        move |zq| callback(Query::from(zq)),
        on_close,
    )
}

#[prebindgen]
pub fn z_session_declare_keyexpr(
    session: &ZSession,
    key_expr: String,
) -> Result<ZKeyExpr, Error> {
    session.declare_keyexpr(key_expr).wait().map_err(Error::from)
}

#[prebindgen]
pub fn z_session_undeclare_keyexpr(session: &ZSession, key_expr: ZKeyExpr) -> Result<(), Error> {
    session.undeclare(key_expr).wait().map_err(Error::from)
}

/// Query matching queryables, delivering each reply as an opaque [`ZReply`]
/// handle (thin surface). `on_close` fires when the reply stream ends.
#[prebindgen]
#[allow(clippy::too_many_arguments)]
pub fn z_session_get(
    session: &ZSession,
    key_expr: impl Into<KeyExpr> + Send + 'static,
    parameters: Option<String>,
    timeout_ms: i64,
    target: QueryTarget,
    consolidation: ConsolidationMode,
    accept_replies: ReplyKeyExpr,
    congestion_control: CongestionControl,
    priority: Priority,
    express: bool,
    payload: Option<impl Into<ZBytes> + Send + 'static>,
    encoding: impl Into<Encoding> + Send + 'static,
    attachment: Option<impl Into<ZBytes> + Send + 'static>,
    callback: impl Fn(ZReply) + Send + Sync + 'static,
    on_close: impl Fn() + Send + Sync + 'static,
) -> Result<(), Error> {
    let ke = into_native(key_expr.into())?;
    let consolidation: zenoh::query::ConsolidationMode = consolidation.into();
    let selector = Selector::owned(&ke, parameters.unwrap_or_default());
    let on_close = OnceDrop::new(on_close);
    let mut builder = session
        .get(selector)
        .congestion_control(congestion_control.into())
        .priority(priority.into())
        .express(express)
        .target(target.into())
        .timeout(Duration::from_millis(timeout_ms as u64))
        .consolidation(consolidation)
        .accept_replies(accept_replies.into());
    if let Some(payload) = payload {
        let payload: ZBytes = payload.into();
        let z_encoding: ZEncoding = encoding.into().try_into()?;
        builder = builder.payload(payload.bytes).encoding(z_encoding);
    }
    if let Some(att) = attachment {
        let att: ZBytes = att.into();
        builder = builder.attachment::<Vec<u8>>(att.bytes);
    }
    builder
        .callback(move |reply| {
            let _ = &on_close;
            callback(reply);
        })
        .wait()
        .map_err(Error::from)
}

/// Query matching queryables, delivering each reply as a fully decoded
/// [`Reply`] data class (thick surface). See [`z_session_get`].
#[prebindgen]
#[allow(clippy::too_many_arguments)]
pub fn session_get(
    session: &ZSession,
    key_expr: impl Into<KeyExpr> + Send + 'static,
    parameters: Option<String>,
    timeout_ms: i64,
    target: QueryTarget,
    consolidation: ConsolidationMode,
    accept_replies: ReplyKeyExpr,
    congestion_control: CongestionControl,
    priority: Priority,
    express: bool,
    payload: Option<impl Into<ZBytes> + Send + 'static>,
    encoding: impl Into<Encoding> + Send + 'static,
    attachment: Option<impl Into<ZBytes> + Send + 'static>,
    callback: impl Fn(Reply) + Send + Sync + 'static,
    on_close: impl Fn() + Send + Sync + 'static,
) -> Result<(), Error> {
    z_session_get(
        session,
        key_expr,
        parameters,
        timeout_ms,
        target,
        consolidation,
        accept_replies,
        congestion_control,
        priority,
        express,
        payload,
        encoding,
        attachment,
        move |zr| callback(Reply::from(&zr)),
        on_close,
    )
}

// Zid accessors return the value-class `ZenohId` directly. With the unified
// newtype projection, value classes ride the same fold-and-wrap machinery as
// opaque handles: the generated Kotlin surface is `ZenohId` / `List<ZenohId>`,
// each value erased to its inner `[B` over the wire and wrapped on the Kotlin
// side, with no raw-bytes hop visible in the FFI surface.
#[prebindgen]
pub fn z_session_zid(session: &ZSession) -> ZenohId {
    ZenohId::from(session.info().zid().wait())
}

#[prebindgen]
pub fn z_session_peers_zid(session: &ZSession) -> Vec<ZenohId> {
    session.info().peers_zid().wait().map(ZenohId::from).collect()
}

#[prebindgen]
pub fn z_session_routers_zid(session: &ZSession) -> Vec<ZenohId> {
    session
        .info()
        .routers_zid()
        .wait()
        .map(ZenohId::from)
        .collect()
}
