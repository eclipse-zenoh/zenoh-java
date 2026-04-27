//! Universal description of how a Rust return value is encoded into the
//! wire-form return value of the wrapper function.

#[derive(Clone)]
pub enum ReturnEncode {
    /// `<path>(&mut env, __result)` — wrapping function returns
    /// `<error_wrapper>(<wire_type>)`. The wrapping function's signature
    /// is destination-language-specific; only the path is captured here.
    Wrapper(syn::Path),
    /// `Ok(Arc::into_raw(Arc::new(__result)))` — opaque Arc-handle return.
    /// Universal across destinations that use `*const T` opaque handles.
    ArcIntoRaw,
}

impl ReturnEncode {
    pub fn wrapper(path: impl AsRef<str>) -> Self {
        ReturnEncode::Wrapper(
            syn::parse_str(path.as_ref()).expect("invalid ReturnEncode::wrapper path"),
        )
    }
}
