//! Conversion closures for function args and return values.
//!
//! * `InputFn` always receives a concrete `syn::Ident` for the input value.
//! * `OutputFn` optionally receives a `syn::Ident` for return-value encoding.

use std::sync::Arc;

use proc_macro2::TokenStream;

#[derive(Clone)]
pub struct InputFn(Option<Arc<dyn Fn(&syn::Ident) -> TokenStream + Send + Sync>>);

impl InputFn {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&syn::Ident) -> TokenStream + Send + Sync + 'static,
    {
        InputFn(Some(Arc::new(f)))
    }

    pub fn unimplemented(_message: impl Into<String>) -> Self {
        InputFn(None)
    }

    pub(crate) fn call(&self, ident: &syn::Ident) -> TokenStream {
        self.0.as_ref().expect("missing input conversion")(ident)
    }
}

pub const NO_INPUT: InputFn = InputFn(None);

#[derive(Clone)]
pub struct OutputFn(Option<Arc<dyn Fn(Option<&syn::Ident>) -> TokenStream + Send + Sync>>);

impl OutputFn {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(Option<&syn::Ident>) -> TokenStream + Send + Sync + 'static,
    {
        OutputFn(Some(Arc::new(f)))
    }

    pub fn unimplemented(_message: impl Into<String>) -> Self {
        OutputFn(None)
    }

    pub(crate) fn call(&self, ident: Option<&syn::Ident>) -> TokenStream {
        self.0.as_ref().expect("missing output conversion")(ident)
    }

    pub(crate) fn is_implemented(&self) -> bool {
        self.0.is_some()
    }
}

pub const NO_OUTPUT: OutputFn = OutputFn(None);

