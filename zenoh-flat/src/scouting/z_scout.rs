use crate::util::OnceDrop;
use crate::{Error, ZConfig, ZHello, ZScout};
use super::hello::Hello;
use prebindgen_proc_macro::prebindgen;
use zenoh::Wait;
use zenoh::config::WhatAmIMatcher;

/// Start a scout, invoking `callback` for each hello message received.
///
/// `whatami` is a bitfield over the [`crate::WhatAmI`] variants
/// (`Router=1 | Peer=2 | Client=4`); only the low 3 bits are
/// significant, the wider type matches the JVM/`Int` matcher
/// representation. When `config` is `None` the default configuration is
/// used.
///
/// `on_close` is dropped — and therefore invoked — when the returned
/// [`ZScout`] is dropped: callers wanting to be notified of scout
/// teardown should attach behavior to that drop.
///
/// Returns an opaque scout handle whose lifetime owns the running scout;
/// dropping it stops the scout and triggers `on_close`.
#[prebindgen]
pub fn z_scout(
    whatami: i32,
    config: Option<&ZConfig>,
    callback: impl Fn(ZHello) + Send + Sync + 'static,
    on_close: impl Fn() + Send + Sync + 'static,
) -> Result<ZScout, Error> {
    let bits = u8::try_from(whatami)
        .map_err(|_| Error { message: format!("invalid whatami bitfield: {whatami}") })?;
    let matcher: WhatAmIMatcher = bits
        .try_into()
        .map_err(|_| Error { message: format!("invalid whatami bitfield: 0b{bits:b}") })?;
    let config = config.cloned().unwrap_or_default();
    let on_close = OnceDrop::new(on_close);
    zenoh::scout(matcher, config)
        .callback(move |hello| {
            let _ = &on_close;
            callback(hello);
        })
        .wait()
        .map_err(Error::from)
}

/// Start a scout, delivering each hello as a serialized [`Hello`] data
/// class. See [`z_scout`] for parameter semantics; this variant pays one
/// extra Rust-side `Hello::from(ZHello)` copy per message in exchange for
/// a single FFI hop on the binding side.
#[prebindgen]
pub fn scout(
    whatami: i32,
    config: Option<&ZConfig>,
    callback: impl Fn(Hello) + Send + Sync + 'static,
    on_close: impl Fn() + Send + Sync + 'static,
) -> Result<ZScout, Error> {
    z_scout(whatami, config, move |zh| callback(Hello::from(zh)), on_close)
}
