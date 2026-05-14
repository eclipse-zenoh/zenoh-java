//! Conversion closures for function args and return values.
//!
//! * `InputFn` always receives a concrete `syn::Ident` for the input value.
//! * `OutputFn` optionally receives a `syn::Ident` for return-value encoding.
//!
//! Transitional: kept alive only because [`crate::core::type_registry`] and
//! [`crate::kotlin::KotlinInterfaceGenerator`] still consume it. Will be
//! removed once Kotlin gen lands on the new Registry.

#![allow(dead_code)]

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

    pub(crate) fn is_implemented(&self) -> bool {
        self.0.is_some()
    }
}

impl<F> From<F> for InputFn
where
    F: Fn(&syn::Ident) -> TokenStream + Send + Sync + 'static,
{
    fn from(f: F) -> Self {
        InputFn::new(f)
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

impl<F> From<F> for OutputFn
where
    F: Fn(Option<&syn::Ident>) -> TokenStream + Send + Sync + 'static,
{
    fn from(f: F) -> Self {
        OutputFn::new(f)
    }
}

pub const NO_OUTPUT: OutputFn = OutputFn(None);

/// Helper to create an [`InputFn`] from a closure.
///
/// # Example
/// ```ignore
/// input_fn(|input: &syn::Ident| -> TokenStream {
///     quote! { #input != 0 }
/// })
/// ```
pub fn input_fn<F>(f: F) -> InputFn
where
    F: Fn(&syn::Ident) -> TokenStream + Send + Sync + 'static,
{
    InputFn::new(f)
}

/// Helper to create an [`OutputFn`] from a closure.
///
/// # Example
/// ```ignore
/// output_fn(|output: Option<&syn::Ident>| -> TokenStream {
///     match output {
///         Some(output) => quote! { #output },
///         None => quote! { null },
///     }
/// })
/// ```
pub fn output_fn<F>(f: F) -> OutputFn
where
    F: Fn(Option<&syn::Ident>) -> TokenStream + Send + Sync + 'static,
{
    OutputFn::new(f)
}
