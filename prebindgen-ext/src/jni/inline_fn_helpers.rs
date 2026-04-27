//! JNI-flavoured constructors for param-direction [`TypeBinding`] rows.
//!
//! These hardcode the `&env` / `&mut env` variable names that match the
//! prelude scope produced by [`JniTryClosureBody`].

use proc_macro2::TokenStream;
use quote::quote;

use crate::core::inline_fn::InputFn;

pub(crate) fn env_ref_mut_decode(path: impl AsRef<str>) -> InputFn {
    let s = path.as_ref().to_string();
    InputFn::new(move |input: &syn::Ident| -> TokenStream {
        let p: syn::Path = syn::parse_str(&s).expect("invalid env_ref_mut path");
        quote! { #p(&mut env, &#input)? }
    })
}
