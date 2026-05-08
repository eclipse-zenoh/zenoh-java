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
///     (|| {
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
/// `TypeBinding`'s `encode`. The closure return type is inferred by Rust.
pub struct JniTryClosureBody {
    pub throw_exception: syn::Path,
}

impl JniTryClosureBody {
    pub fn new(throw_exception: impl AsRef<str>) -> Self {
        Self {
            throw_exception: syn::parse_str(throw_exception.as_ref())
                .expect("invalid throw_exception path"),
        }
    }
}

impl BodyStrategy for JniTryClosureBody {
    fn build_body(&self, ctx: BodyContext) -> TokenStream {
        let prelude = ctx.prelude;
        let call = &ctx.call_expr;
        let throw = &self.throw_exception;
        let result_ident = format_ident!("__result");

        let (wrap_ok, on_err): (TokenStream, TokenStream) =
            match (ctx.wire_return, ctx.return_encode) {
                (None, _) => (
                    quote! { Ok(()) },
                    quote! { () },
                ),
                (Some(_wire), Some(encode)) => {
                    let encoded = encode.call(Some(&result_ident));
                    let on_err = encode.call(None);
                    (
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
                (|| {
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
