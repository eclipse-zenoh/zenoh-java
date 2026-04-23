use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};

fn main() {
    let source = prebindgen::Source::new(zenoh_flat::PREBINDGEN_OUT_DIR);

    source
        .items_all()
        .map(|(item, loc)| (jni_convert(item, &loc), loc))
        .collect::<prebindgen::collect::Destination>()
        .write("zenoh_flat_jni.rs");
}

fn jni_convert(item: syn::Item, loc: &prebindgen::SourceLocation) -> syn::Item {
    let syn::Item::Fn(func) = item else {
        return item;
    };
    syn::Item::Fn(convert_fn(func, loc))
}

fn convert_fn(func: syn::ItemFn, loc: &prebindgen::SourceLocation) -> syn::ItemFn {
    let original_name = func.sig.ident.to_string();
    let jni_name = format_ident!(
        "Java_io_zenoh_jni_JNISession_{}ViaJNI",
        snake_to_camel(&original_name)
    );
    let orig_ident = &func.sig.ident;

    let mut prelude: Vec<TokenStream> = Vec::new();
    let mut jni_params: Vec<TokenStream> = Vec::new();
    let mut call_args: Vec<TokenStream> = Vec::new();

    for input in &func.sig.inputs {
        let syn::FnArg::Typed(pat_type) = input else {
            panic!("receiver args not supported at {loc}");
        };
        let syn::Pat::Ident(pat_ident) = &*pat_type.pat else {
            panic!("non-ident param pattern at {loc}");
        };
        let name = &pat_ident.ident;
        let ty = &*pat_type.ty;

        match classify_arg(ty) {
            ArgKind::OpaqueRef(elem) => {
                let ptr_ident = format_ident!("{}_ptr", name);
                jni_params.push(quote! { #ptr_ident: *const #elem });
                prelude.push(quote! {
                    let #name = crate::owned_object::OwnedObject::from_raw(#ptr_ident);
                });
                call_args.push(quote! { &#name });
            }
            ArgKind::KeyExpr => {
                let ptr_ident = format_ident!("{}_ptr", name);
                let str_ident = format_ident!("{}_str", name);
                jni_params.push(quote! {
                    #ptr_ident: *const zenoh::key_expr::KeyExpr<'static>
                });
                jni_params.push(quote! { #str_ident: jni::objects::JString });
                prelude.push(quote! {
                    let #name = crate::key_expr::process_kotlin_key_expr(
                        &mut env, &#str_ident, #ptr_ident,
                    )?;
                });
                call_args.push(quote! { #name });
            }
            ArgKind::Enum(decoder) => {
                jni_params.push(quote! { #name: jni::sys::jint });
                prelude.push(quote! {
                    let #name = crate::utils::#decoder(#name)?;
                });
                call_args.push(quote! { #name });
            }
            ArgKind::Bool => {
                jni_params.push(quote! { #name: jni::sys::jboolean });
                prelude.push(quote! { let #name = #name != 0; });
                call_args.push(quote! { #name });
            }
            ArgKind::Unsupported => panic!(
                "unsupported parameter type `{}` for `{}` at {loc}",
                ty.to_token_stream(),
                name
            ),
        }
    }

    let (ret_ty_jni, wrap_ok, on_err, closure_ret): (
        TokenStream,
        TokenStream,
        TokenStream,
        TokenStream,
    ) = match &func.sig.output {
        syn::ReturnType::Type(_, ty) => {
            let inner = extract_zresult_inner(ty)
                .unwrap_or_else(|| panic!("return must be ZResult<T> for `{original_name}`"));
            if is_unit(&inner) {
                (
                    quote! { () },
                    quote! { Ok(()) },
                    quote! { () },
                    quote! { crate::errors::ZResult<()> },
                )
            } else {
                (
                    quote! { *const #inner },
                    quote! { Ok(std::sync::Arc::into_raw(std::sync::Arc::new(__result))) },
                    quote! { std::ptr::null() },
                    quote! { crate::errors::ZResult<*const #inner> },
                )
            }
        }
        syn::ReturnType::Default => (
            quote! { () },
            quote! { Ok(()) },
            quote! { () },
            quote! { crate::errors::ZResult<()> },
        ),
    };

    let body = quote! {
        {
            (|| -> #closure_ret {
                #(#prelude)*
                let __result = zenoh_flat::session::#orig_ident( #(#call_args),* )?;
                #wrap_ok
            })()
            .unwrap_or_else(|err| {
                crate::throw_exception!(env, err);
                #on_err
            })
        }
    };

    let tokens = quote! {
        #[no_mangle]
        #[allow(non_snake_case, unused_mut, unused_variables)]
        pub unsafe extern "C" fn #jni_name(
            mut env: jni::JNIEnv,
            _class: jni::objects::JClass,
            #(#jni_params),*
        ) -> #ret_ty_jni #body
    };

    syn::parse2(tokens).expect("generated JNI wrapper must parse")
}

enum ArgKind {
    /// `&T` (excluding `&KeyExpr`) — handle arg via `OwnedObject::from_raw`.
    OpaqueRef(syn::Type),
    /// `KeyExpr<'static>` — two JNI args (ptr + JString) via `process_kotlin_key_expr`.
    KeyExpr,
    /// Enum type passed as `jint` ordinal, with matching `decode_*` helper.
    Enum(syn::Ident),
    /// `bool` → `jboolean`.
    Bool,
    Unsupported,
}

fn classify_arg(ty: &syn::Type) -> ArgKind {
    match ty {
        syn::Type::Reference(r) if r.mutability.is_none() => {
            if type_last_segment(&r.elem).map(|s| s == "KeyExpr").unwrap_or(false) {
                ArgKind::KeyExpr
            } else {
                ArgKind::OpaqueRef((*r.elem).clone())
            }
        }
        syn::Type::Path(tp) => {
            let Some(last) = tp.path.segments.last() else {
                return ArgKind::Unsupported;
            };
            let name = last.ident.to_string();
            if name == "bool" {
                return ArgKind::Bool;
            }
            if name == "KeyExpr" {
                return ArgKind::KeyExpr;
            }
            if let Some(decoder) = enum_decoder(&name) {
                return ArgKind::Enum(format_ident!("{}", decoder));
            }
            ArgKind::Unsupported
        }
        _ => ArgKind::Unsupported,
    }
}

fn enum_decoder(type_name: &str) -> Option<&'static str> {
    Some(match type_name {
        "CongestionControl" => "decode_congestion_control",
        "Priority" => "decode_priority",
        "Reliability" => "decode_reliability",
        "QueryTarget" => "decode_query_target",
        "ConsolidationMode" => "decode_consolidation",
        "ReplyKeyExpr" => "decode_reply_key_expr",
        _ => return None,
    })
}

fn type_last_segment(ty: &syn::Type) -> Option<String> {
    let syn::Type::Path(tp) = ty else { return None };
    tp.path.segments.last().map(|s| s.ident.to_string())
}

fn is_unit(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Tuple(t) if t.elems.is_empty())
}

fn extract_zresult_inner(ty: &syn::Type) -> Option<syn::Type> {
    let syn::Type::Path(tp) = ty else {
        return None;
    };
    let seg = tp.path.segments.last()?;
    if seg.ident != "ZResult" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &seg.arguments else {
        return None;
    };
    let arg = args.args.first()?;
    let syn::GenericArgument::Type(inner) = arg else {
        return None;
    };
    Some(inner.clone())
}

fn snake_to_camel(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut upper_next = false;
    for (i, c) in s.chars().enumerate() {
        if c == '_' {
            upper_next = true;
        } else if upper_next {
            out.extend(c.to_uppercase());
            upper_next = false;
        } else if i == 0 {
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}
