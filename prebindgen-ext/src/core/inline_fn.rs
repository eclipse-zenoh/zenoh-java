//! Conversion closures for function args and return values.
//!
//! * `InputFn` always receives a concrete `syn::Ident` for the input value.
//! * `OutputFn` optionally receives a `syn::Ident` for return-value encoding.

use std::sync::Arc;

use proc_macro2::TokenStream;

#[derive(Clone)]
pub struct InputFn(Arc<dyn Fn(&syn::Ident) -> TokenStream + Send + Sync>);

impl InputFn {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&syn::Ident) -> TokenStream + Send + Sync + 'static,
    {
        InputFn(Arc::new(f))
    }

    pub(crate) fn call(&self, ident: &syn::Ident) -> TokenStream {
        (self.0)(ident)
    }
}

#[derive(Clone)]
pub struct OutputFn(Arc<dyn Fn(Option<&syn::Ident>) -> TokenStream + Send + Sync>);

impl OutputFn {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(Option<&syn::Ident>) -> TokenStream + Send + Sync + 'static,
    {
        OutputFn(Arc::new(f))
    }

    pub(crate) fn call(&self, ident: Option<&syn::Ident>) -> TokenStream {
        (self.0)(ident)
    }
}

/// Backward-compatible alias for the former generic inline conversion
/// closure type. Existing input-only call sites can continue to use
/// `InlineFn`.
pub type InlineFn = InputFn;
