//! JNI-flavoured constructors for param-direction [`TypeBinding`] rows.
//!
//! These hardcode the `&env` / `&mut env` variable names that match the
//! prelude scope produced by [`JniTryClosureBody`].

use proc_macro2::TokenStream;
use quote::quote;

use crate::core::inline_fn::InlineFn;

pub(crate) fn env_ref_mut_decode(path: impl AsRef<str>) -> InlineFn {
    let s = path.as_ref().to_string();
    InlineFn::new(move |input: Option<&syn::Ident>| -> TokenStream {
        let input = input.expect("env_ref_mut_decode requires an input ident");
        let p: syn::Path = syn::parse_str(&s).expect("invalid env_ref_mut path");
        quote! { #p(&mut env, &#input)? }
    })
}
