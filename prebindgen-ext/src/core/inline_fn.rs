//! Universal `InlineFn` — a clonable closure that produces a `TokenStream`
//! from an optional `syn::Ident`.
//!
//! Language-flavoured constructors (e.g. JNI's `env_ref` / `env_ref_mut`)
//! live in `prebindgen-ext::jni::inline_fn_helpers`. This module exposes
//! only the raw constructor `InlineFn::new`.

use std::sync::Arc;

use proc_macro2::TokenStream;

#[derive(Clone)]
pub struct InlineFn(Arc<dyn Fn(Option<&syn::Ident>) -> TokenStream + Send + Sync>);

impl InlineFn {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(Option<&syn::Ident>) -> TokenStream + Send + Sync + 'static,
    {
        InlineFn(Arc::new(f))
    }

    pub(crate) fn call(&self, ident: Option<&syn::Ident>) -> TokenStream {
        (self.0)(ident)
    }
}
