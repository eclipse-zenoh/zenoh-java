//! JNI-flavoured constructors for param-direction [`TypeBinding`] rows.
//!
//! These hardcode the `&env` / `&mut env` variable names that match the
//! prelude scope produced by [`JniTryClosureBody`].

use proc_macro2::TokenStream;
use quote::quote;

use crate::core::inline_fn::InlineFn;
use crate::core::type_binding::TypeBinding;

/// `<path>(<input>)?` — pure conversion (e.g. enum decoders).
pub(crate) fn pure(
    rust_type: impl AsRef<str>,
    jni_type: impl AsRef<str>,
    path: impl AsRef<str>,
) -> TypeBinding {
    TypeBinding::input(rust_type, jni_type, pure_decode(path))
}

pub(crate) fn pure_decode(path: impl AsRef<str>) -> InlineFn {
    let s = path.as_ref().to_string();
    InlineFn::new(move |input: Option<&syn::Ident>| -> TokenStream {
        let input = input.expect("pure_decode requires an input ident");
        let p: syn::Path = syn::parse_str(&s).expect("invalid pure path");
        quote! { #p(#input)? }
    })
}

/// `<path>(&env, &<input>)?` — decoder needing shared access to the JNI env.
pub(crate) fn env_ref(
    rust_type: impl AsRef<str>,
    jni_type: impl AsRef<str>,
    path: impl AsRef<str>,
) -> TypeBinding {
    TypeBinding::input(rust_type, jni_type, env_ref_decode(path))
}

pub(crate) fn env_ref_decode(path: impl AsRef<str>) -> InlineFn {
    let s = path.as_ref().to_string();
    InlineFn::new(move |input: Option<&syn::Ident>| -> TokenStream {
        let input = input.expect("env_ref_decode requires an input ident");
        let p: syn::Path = syn::parse_str(&s).expect("invalid env_ref path");
        quote! { #p(&env, &#input)? }
    })
}

/// `<path>(&mut env, &<input>)?` — decoder needing mutable access to the JNI env.
pub(crate) fn env_ref_mut(
    rust_type: impl AsRef<str>,
    jni_type: impl AsRef<str>,
    path: impl AsRef<str>,
) -> TypeBinding {
    TypeBinding::input(rust_type, jni_type, env_ref_mut_decode(path))
}

pub(crate) fn env_ref_mut_decode(path: impl AsRef<str>) -> InlineFn {
    let s = path.as_ref().to_string();
    InlineFn::new(move |input: Option<&syn::Ident>| -> TokenStream {
        let input = input.expect("env_ref_mut_decode requires an input ident");
        let p: syn::Path = syn::parse_str(&s).expect("invalid env_ref_mut path");
        quote! { #p(&mut env, &#input)? }
    })
}
