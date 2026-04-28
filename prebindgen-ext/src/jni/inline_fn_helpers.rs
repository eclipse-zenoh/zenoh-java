//! JNI-flavoured constructors for param-direction [`TypeBinding`] rows.
//!
//! These hardcode the `&env` / `&mut env` variable names that match the
//! prelude scope produced by [`JniTryClosureBody`].

use proc_macro2::TokenStream;
use quote::quote;

use crate::core::inline_fn::InputFn;
use crate::core::inline_fn::OutputFn;

pub(crate) fn env_ref_mut_decode(path: impl AsRef<str>) -> InputFn {
    let s = path.as_ref().to_string();
    InputFn::new(move |input: &syn::Ident| -> TokenStream {
        let p: syn::Path = syn::parse_str(&s).expect("invalid env_ref_mut path");
        quote! { #p(&mut env, &#input)? }
    })
}

pub(crate) fn env_ref_mut_encode(path: impl AsRef<str>) -> OutputFn {
    let s = path.as_ref().to_string();
    OutputFn::new(move |output: Option<&syn::Ident>| -> TokenStream {
        let p: syn::Path = syn::parse_str(&s).expect("invalid env_ref_mut_encode path");
        match output {
            Some(output) => quote! { #p(&mut env, #output)? },
            None => quote! { std::ptr::null_mut() },
        }
    })
}
