//! `BodyStrategy` impl that wraps the wrapped-fn call in a try-closure
//! and routes errors through `throw_exception!`.

use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};

use crate::core::functions_converter::{BodyContext, BodyStrategy};

/// JNI try-closure body strategy.
///
/// Produces:
/// ```ignore
/// {
///     (|| -> <zresult>(<wire_return or ()>) {
///         <prelude>
///         let __result = <call_expr>?;
///         <wrap_ok>
///     })()
///     .unwrap_or_else(|err| {
///         <throw_exception>!(env, err);
///         <on_err>
///     })
/// }
/// ```
/// where `wrap_ok` and `on_err` are derived from the return-direction
/// `TypeBinding`'s `encode`.
pub struct JniTryClosureBody {
    pub zresult: syn::Path,
    pub throw_exception: syn::Path,
}

impl JniTryClosureBody {
    pub fn new(zresult: impl AsRef<str>, throw_exception: impl AsRef<str>) -> Self {
        Self {
            zresult: syn::parse_str(zresult.as_ref()).expect("invalid zresult path"),
            throw_exception: syn::parse_str(throw_exception.as_ref())
                .expect("invalid throw_exception path"),
        }
    }
}

impl BodyStrategy for JniTryClosureBody {
    fn build_body(&self, ctx: BodyContext) -> TokenStream {
        let prelude = ctx.prelude;
        let call = &ctx.call_expr;
        let zresult = &self.zresult;
        let throw = &self.throw_exception;
        let result_ident = format_ident!("__result");

        let (closure_ret, wrap_ok, on_err): (TokenStream, TokenStream, TokenStream) =
            match (ctx.wire_return, ctx.return_encode) {
                (None, _) => (
                    quote! { #zresult<()> },
                    quote! { Ok(()) },
                    quote! { () },
                ),
                (Some(wire), Some(encode)) => {
                    let encoded = encode.call(Some(&result_ident));
                    let on_err = encode.call(None);
                    (
                        quote! { #zresult<#wire> },
                        quote! { Ok(#encoded) },
                        quote! { #on_err },
                    )
                }
                (Some(wire), None) => {
                    panic!(
                        "JniTryClosureBody: wire return `{}` has no encode at {}",
                        wire.to_token_stream(),
                        ctx.loc
                    );
                }
            };

        quote! {
            {
                (|| -> #closure_ret {
                    #(#prelude)*
                    let #result_ident = #call?;
                    #wrap_ok
                })()
                .unwrap_or_else(|err| {
                    #throw!(env, err);
                    #on_err
                })
            }
        }
    }
}
