//! JNI binding generator for functions marked with `#[prebindgen]`.
//!
//! This module mirrors the pattern of [`prebindgen::batching::FfiConverter`], but
//! instead of emitting `#[no_mangle] extern "C"` proxy functions, it emits
//! `Java_<class>_<name>ViaJNI` wrappers that decode JNI arguments, call the
//! original Rust function, and wrap the result into a raw pointer (or throw a
//! JVM exception on error).
//!
//! # Pipeline
//!
//! ```ignore
//! use itertools::Itertools;
//! let source = prebindgen::Source::new(zenoh_flat::PREBINDGEN_OUT_DIR);
//! let converter = zenoh_flat::jni_converter::JniConverter::builder()
//!     .class_prefix("Java_io_zenoh_jni_JNISession_")
//!     .function_suffix("ViaJNI")
//!     .source_module("zenoh_flat::session")
//!     .owned_object("crate::owned_object::OwnedObject")
//!     .zresult("crate::errors::ZResult")
//!     .throw_exception("crate::throw_exception")
//!     .key_expr_decoder("crate::key_expr::process_kotlin_key_expr")
//!     .enum_decoder("CongestionControl", "crate::utils::decode_congestion_control")
//!     .build();
//! source
//!     .items_all()
//!     .batching(converter.into_closure())
//!     .collect::<prebindgen::collect::Destination>()
//!     .write("zenoh_flat_jni.rs");
//! ```
//!
//! This crate is currently coupled to zenoh-jni's type layout (e.g. `KeyExpr`,
//! decoder helpers, `OwnedObject`) — those couplings are configurable through
//! the [`Builder`] so the converter itself stays data-driven.

use std::collections::{HashMap, VecDeque};

use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};

use prebindgen::SourceLocation;

/// Builder for [`JniConverter`].
pub struct Builder {
    class_prefix: String,
    function_suffix: String,
    source_module: syn::Path,
    owned_object: syn::Path,
    zresult: syn::Path,
    throw_exception: syn::Path,
    key_expr_decoder: Option<syn::Path>,
    byte_array_decoder: Option<syn::Path>,
    enum_decoders: HashMap<String, syn::Path>,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            class_prefix: String::new(),
            function_suffix: String::new(),
            source_module: syn::parse_str("crate").unwrap(),
            owned_object: syn::parse_str("OwnedObject").unwrap(),
            zresult: syn::parse_str("ZResult").unwrap(),
            throw_exception: syn::parse_str("throw_exception").unwrap(),
            key_expr_decoder: None,
            byte_array_decoder: None,
            enum_decoders: HashMap::new(),
        }
    }
}

impl Builder {
    /// JNI class prefix prepended to each function name, e.g.
    /// `"Java_io_zenoh_jni_JNISession_"`.
    pub fn class_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.class_prefix = prefix.into();
        self
    }

    /// Suffix appended to the camel-case function name, e.g. `"ViaJNI"`.
    pub fn function_suffix(mut self, suffix: impl Into<String>) -> Self {
        self.function_suffix = suffix.into();
        self
    }

    /// Fully-qualified path of the module that contains the original Rust
    /// functions being wrapped, e.g. `"zenoh_flat::session"`.
    pub fn source_module(mut self, path: impl AsRef<str>) -> Self {
        self.source_module = syn::parse_str(path.as_ref()).expect("invalid source_module path");
        self
    }

    /// Path of the `OwnedObject` helper used to borrow Arc-pointers.
    pub fn owned_object(mut self, path: impl AsRef<str>) -> Self {
        self.owned_object = syn::parse_str(path.as_ref()).expect("invalid owned_object path");
        self
    }

    /// Path of the `ZResult` type used in the closure's return type.
    pub fn zresult(mut self, path: impl AsRef<str>) -> Self {
        self.zresult = syn::parse_str(path.as_ref()).expect("invalid zresult path");
        self
    }

    /// Path of the `throw_exception!` macro (will be called as `<path>!`).
    pub fn throw_exception(mut self, path: impl AsRef<str>) -> Self {
        self.throw_exception =
            syn::parse_str(path.as_ref()).expect("invalid throw_exception path");
        self
    }

    /// Path of the function that decodes a `KeyExpr` from a `(ptr, JString)`
    /// pair, e.g. `"crate::key_expr::process_kotlin_key_expr"`.
    pub fn key_expr_decoder(mut self, path: impl AsRef<str>) -> Self {
        self.key_expr_decoder =
            Some(syn::parse_str(path.as_ref()).expect("invalid key_expr_decoder path"));
        self
    }

    /// Path of the function that decodes a `JByteArray` into `Vec<u8>`, e.g.
    /// `"crate::utils::decode_byte_array"`. Used for both `Vec<u8>` and
    /// `Option<Vec<u8>>` parameters.
    pub fn byte_array_decoder(mut self, path: impl AsRef<str>) -> Self {
        self.byte_array_decoder =
            Some(syn::parse_str(path.as_ref()).expect("invalid byte_array_decoder path"));
        self
    }

    /// Register a decoder for an enum type. `type_name` is matched against the
    /// last segment of the parameter's type path.
    pub fn enum_decoder(
        mut self,
        type_name: impl Into<String>,
        decoder: impl AsRef<str>,
    ) -> Self {
        let path: syn::Path =
            syn::parse_str(decoder.as_ref()).expect("invalid enum_decoder path");
        self.enum_decoders.insert(type_name.into(), path);
        self
    }

    pub fn build(self) -> JniConverter {
        JniConverter {
            cfg: self,
            pending: VecDeque::new(),
        }
    }
}

/// Converter that transforms `#[prebindgen]`-marked Rust functions into JNI
/// `Java_*` wrappers.
///
/// Intended for use with `itertools::batching`:
///
/// ```ignore
/// source.items_all().batching(converter.into_closure())
/// ```
pub struct JniConverter {
    cfg: Builder,
    pending: VecDeque<(syn::Item, SourceLocation)>,
}

impl JniConverter {
    pub fn builder() -> Builder {
        Builder::default()
    }

    /// Pull one item from `iter`, convert it, and return it. Non-function
    /// items are passed through unchanged. Returns `None` once `iter` is
    /// exhausted and no buffered items remain.
    pub fn call<I>(&mut self, iter: &mut I) -> Option<(syn::Item, SourceLocation)>
    where
        I: Iterator<Item = (syn::Item, SourceLocation)>,
    {
        if let Some(buf) = self.pending.pop_front() {
            return Some(buf);
        }
        let (item, loc) = iter.next()?;
        Some((self.convert(item, &loc), loc))
    }

    /// Closure suitable for `itertools::batching`.
    pub fn into_closure<I>(
        mut self,
    ) -> impl FnMut(&mut I) -> Option<(syn::Item, SourceLocation)>
    where
        I: Iterator<Item = (syn::Item, SourceLocation)>,
    {
        move |iter| self.call(iter)
    }

    fn convert(&self, item: syn::Item, loc: &SourceLocation) -> syn::Item {
        match item {
            syn::Item::Fn(func) => syn::Item::Fn(self.convert_fn(func, loc)),
            other => other,
        }
    }

    fn convert_fn(&self, func: syn::ItemFn, loc: &SourceLocation) -> syn::ItemFn {
        let original_name = func.sig.ident.to_string();
        let camel = snake_to_camel(&original_name);
        let jni_name = format_ident!("{}{}{}", self.cfg.class_prefix, camel, self.cfg.function_suffix);
        let orig_ident = &func.sig.ident;
        let source_module = &self.cfg.source_module;
        let owned_object = &self.cfg.owned_object;
        let zresult = &self.cfg.zresult;
        let throw_exception = &self.cfg.throw_exception;

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

            match self.classify_arg(ty) {
                ArgKind::OpaqueRef(elem) => {
                    let ptr_ident = format_ident!("{}_ptr", name);
                    jni_params.push(quote! { #ptr_ident: *const #elem });
                    prelude.push(quote! {
                        let #name = #owned_object::from_raw(#ptr_ident);
                    });
                    call_args.push(quote! { &#name });
                }
                ArgKind::KeyExpr => {
                    let decoder = self
                        .cfg
                        .key_expr_decoder
                        .as_ref()
                        .expect("key_expr_decoder not configured");
                    let ptr_ident = format_ident!("{}_ptr", name);
                    let str_ident = format_ident!("{}_str", name);
                    jni_params.push(quote! {
                        #ptr_ident: *const zenoh::key_expr::KeyExpr<'static>
                    });
                    jni_params.push(quote! { #str_ident: jni::objects::JString });
                    prelude.push(quote! {
                        let #name = #decoder(&mut env, &#str_ident, #ptr_ident)?;
                    });
                    call_args.push(quote! { #name });
                }
                ArgKind::Enum(decoder) => {
                    jni_params.push(quote! { #name: jni::sys::jint });
                    prelude.push(quote! {
                        let #name = #decoder(#name)?;
                    });
                    call_args.push(quote! { #name });
                }
                ArgKind::Bool => {
                    jni_params.push(quote! { #name: jni::sys::jboolean });
                    prelude.push(quote! { let #name = #name != 0; });
                    call_args.push(quote! { #name });
                }
                ArgKind::Duration => {
                    jni_params.push(quote! { #name: jni::sys::jlong });
                    prelude.push(quote! {
                        let #name = std::time::Duration::from_millis(#name as u64);
                    });
                    call_args.push(quote! { #name });
                }
                ArgKind::OptionVecU8 => {
                    let decoder = self
                        .cfg
                        .byte_array_decoder
                        .as_ref()
                        .expect("byte_array_decoder not configured");
                    jni_params.push(quote! { #name: jni::objects::JByteArray });
                    prelude.push(quote! {
                        let #name = if !#name.is_null() {
                            Some(#decoder(&env, #name)?)
                        } else {
                            None
                        };
                    });
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
                let inner = extract_zresult_inner(ty).unwrap_or_else(|| {
                    panic!("return must be ZResult<T> for `{original_name}` at {loc}")
                });
                if is_unit(&inner) {
                    (
                        quote! { () },
                        quote! { Ok(()) },
                        quote! { () },
                        quote! { #zresult<()> },
                    )
                } else {
                    (
                        quote! { *const #inner },
                        quote! { Ok(std::sync::Arc::into_raw(std::sync::Arc::new(__result))) },
                        quote! { std::ptr::null() },
                        quote! { #zresult<*const #inner> },
                    )
                }
            }
            syn::ReturnType::Default => (
                quote! { () },
                quote! { Ok(()) },
                quote! { () },
                quote! { #zresult<()> },
            ),
        };

        let body = quote! {
            {
                (|| -> #closure_ret {
                    #(#prelude)*
                    let __result = #source_module::#orig_ident( #(#call_args),* )?;
                    #wrap_ok
                })()
                .unwrap_or_else(|err| {
                    #throw_exception!(env, err);
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

    fn classify_arg(&self, ty: &syn::Type) -> ArgKind {
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
                if name == "Duration" {
                    return ArgKind::Duration;
                }
                if name == "Option" && is_option_of_vec_u8(last) {
                    return ArgKind::OptionVecU8;
                }
                if let Some(decoder) = self.cfg.enum_decoders.get(&name) {
                    return ArgKind::Enum(decoder.clone());
                }
                ArgKind::Unsupported
            }
            _ => ArgKind::Unsupported,
        }
    }
}

enum ArgKind {
    OpaqueRef(syn::Type),
    KeyExpr,
    Enum(syn::Path),
    Bool,
    Duration,
    /// `Option<Vec<u8>>` → `JByteArray` decoded via `byte_array_decoder`.
    OptionVecU8,
    Unsupported,
}

fn type_last_segment(ty: &syn::Type) -> Option<String> {
    let syn::Type::Path(tp) = ty else { return None };
    tp.path.segments.last().map(|s| s.ident.to_string())
}

/// Check whether an `Option<...>` path segment wraps exactly `Vec<u8>`.
fn is_option_of_vec_u8(seg: &syn::PathSegment) -> bool {
    let syn::PathArguments::AngleBracketed(args) = &seg.arguments else {
        return false;
    };
    let Some(syn::GenericArgument::Type(inner)) = args.args.first() else {
        return false;
    };
    let syn::Type::Path(inner_path) = inner else {
        return false;
    };
    let Some(inner_seg) = inner_path.path.segments.last() else {
        return false;
    };
    if inner_seg.ident != "Vec" {
        return false;
    }
    let syn::PathArguments::AngleBracketed(vec_args) = &inner_seg.arguments else {
        return false;
    };
    let Some(syn::GenericArgument::Type(elem)) = vec_args.args.first() else {
        return false;
    };
    matches!(
        elem,
        syn::Type::Path(tp) if tp.path.is_ident("u8")
    )
}

fn is_unit(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Tuple(t) if t.elems.is_empty())
}

fn extract_zresult_inner(ty: &syn::Type) -> Option<syn::Type> {
    let syn::Type::Path(tp) = ty else { return None };
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
