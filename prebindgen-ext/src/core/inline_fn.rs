//! Conversion closures for function args and return values.
//!
//! * `InputFn` always receives a concrete `syn::Ident` for the input value.
//! * `OutputFn` optionally receives a `syn::Ident` for return-value encoding.

use std::sync::Arc;

use proc_macro2::{Span, TokenStream};
use quote::quote;

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

/// Wraps an [`InputFn`] for `T` into one for `Option<T>`.
/// The wire value must expose an `.is_null()` method (e.g. JNI reference types);
/// a truthy result maps to `None`, otherwise the inner conversion is applied.
pub fn option_input(inner: InputFn) -> InputFn {
    InputFn::new(move |input: &syn::Ident| -> TokenStream {
        let inner_expr = inner.call(input);
        quote! {
            if !#input.is_null() {
                Some(#inner_expr)
            } else {
                None
            }
        }
    })
}

/// Wraps an [`OutputFn`] for `T` into one for `Option<T>`.
/// The `None` arm of the inner function is reused as the null wire value,
/// so no separate null-sentinel helper is needed here.
pub fn option_output(inner: OutputFn) -> OutputFn {
    OutputFn::new(move |output: Option<&syn::Ident>| -> TokenStream {
        let null_expr = inner.call(None);
        match output {
            Some(output) => {
                let value_ident = syn::Ident::new("value", Span::call_site());
                let inner_expr = inner.call(Some(&value_ident));
                quote! {
                    match &#output {
                        Some(value) => #inner_expr,
                        None => #null_expr,
                    }
                }
            }
            None => null_expr,
        }
    })
}

