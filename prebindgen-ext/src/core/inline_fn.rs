//! Universal `InlineFn` — a clonable closure that produces a `TokenStream`
//! from a `syn::Ident` (the wire-form input variable name).
//!
//! Language-flavoured constructors (e.g. JNI's `env_ref` / `env_ref_mut`)
//! live in `prebindgen-ext::jni::inline_fn_helpers`. This module exposes
//! only the raw constructor `InlineFn::new`.

use std::sync::Arc;

use proc_macro2::TokenStream;

#[derive(Clone)]
pub struct InlineFn(Arc<dyn Fn(&syn::Ident) -> TokenStream + Send + Sync>);

impl InlineFn {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&syn::Ident) -> TokenStream + Send + Sync + 'static,
    {
        InlineFn(Arc::new(f))
    }

    pub(crate) fn call(&self, ident: &syn::Ident) -> TokenStream {
        (self.0)(ident)
    }
}
