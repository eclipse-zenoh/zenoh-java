//! JNI-flavoured constructors for [`InlineFn`].
//!
//! These hardcode the `&env` / `&mut env` variable names that match the
//! prelude scope produced by [`JniTryClosureBody`].

use proc_macro2::TokenStream;
use quote::quote;

use crate::core::inline_fn::InlineFn;

/// `<path>(<input>)?` — pure conversion (e.g. enum decoders).
pub fn pure(path: impl AsRef<str>) -> InlineFn {
    let s = path.as_ref().to_string();
    InlineFn::new(move |input: &syn::Ident| -> TokenStream {
        let p: syn::Path = syn::parse_str(&s).expect("invalid pure path");
        quote! { #p(#input)? }
    })
}

/// `<path>(&env, &<input>)?` — decoder needing shared access to the JNI env.
pub fn env_ref(path: impl AsRef<str>) -> InlineFn {
    let s = path.as_ref().to_string();
    InlineFn::new(move |input: &syn::Ident| -> TokenStream {
        let p: syn::Path = syn::parse_str(&s).expect("invalid env_ref path");
        quote! { #p(&env, &#input)? }
    })
}

/// `<path>(&mut env, &<input>)?` — decoder needing mutable access to the JNI env.
pub fn env_ref_mut(path: impl AsRef<str>) -> InlineFn {
    let s = path.as_ref().to_string();
    InlineFn::new(move |input: &syn::Ident| -> TokenStream {
        let p: syn::Path = syn::parse_str(&s).expect("invalid env_ref_mut path");
        quote! { #p(&mut env, &#input)? }
    })
}
