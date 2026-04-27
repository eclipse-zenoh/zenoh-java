//! Convenience [`TypeBinding`] constructors for opaque-handle JNI patterns.

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};

use crate::core::inline_fn::InlineFn;
use crate::core::type_binding::{jni_object_shaped, TypeBinding};

/// Opaque borrow `&T` — JNI side passes raw `*const T`, decoded via
/// `<owned_object>::from_raw`. Because the row's key starts with `&`,
/// the wrapped fn receives `&name` automatically.
pub fn opaque_borrow(t: impl AsRef<str>, owned_object: impl AsRef<str>) -> TypeBinding {
    let t = t.as_ref().to_string();
    let owned_str = owned_object.as_ref().to_string();
    // Validate the owner path parses now so errors surface at registration.
    let _: syn::Path =
        syn::parse_str(&owned_str).expect("opaque_borrow: invalid owned_object path");
    TypeBinding::input(
        format!("&{}", t),
        format!("*const {}", t),
        InlineFn::new(move |input: Option<&syn::Ident>| -> TokenStream {
            let input = input.expect("opaque_borrow decode requires an input ident");
            let owned: syn::Path =
                syn::parse_str(&owned_str).expect("owned_object must parse");
            quote! { #owned::from_raw(#input) }
        }),
    )
}

/// Opaque Arc return for `ZResult<T>` — encode via
/// `Arc::into_raw(Arc::new(__result))`, default to `std::ptr::null()`.
pub fn opaque_arc_return(t: impl AsRef<str>) -> TypeBinding {
    let t = t.as_ref().to_string();
    TypeBinding::output(
        format!("ZResult<{}>", t),
        format!("*const {}", t),
        InlineFn::new(move |output: Option<&syn::Ident>| -> TokenStream {
            match output {
                Some(output) => {
                    quote! { std::sync::Arc::into_raw(std::sync::Arc::new(#output)) }
                }
                None => quote! { std::ptr::null() },
            }
        }),
    )
}

/// `Option<X>` row that lifts `inner`'s decode with a JNI-side null
/// check. Inner's wire form must be JNI-object-shaped (`JObject`,
/// `JString`, or `JByteArray`) — those are the wire types that support
/// `is_null()`.
pub fn option_of_jobject(inner: &TypeBinding) -> TypeBinding {
    let inner_decode = inner
        .decode
        .clone()
        .expect("option_of_jobject: inner must be a param row");
    assert!(
        jni_object_shaped(&inner.wire_type),
        "option_of_jobject requires a JNI-object inner wire form, got `{}`",
        inner.wire_type.to_token_stream()
    );
    TypeBinding::input_output(
        format!("Option<{}>", &inner.rust_type),
        inner.wire_type.clone(),
        Some(InlineFn::new(move |input: Option<&syn::Ident>| -> TokenStream {
            let input = input.expect("option_of_jobject decode requires an input ident");
            let inner_expr = inner_decode.call(Some(input));
            quote! {
                if !#input.is_null() {
                    Some(#inner_expr)
                } else {
                    None
                }
            }
        })),
        None,
    )
}
