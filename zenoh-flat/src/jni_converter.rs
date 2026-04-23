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

use std::collections::{HashMap, HashSet, VecDeque};

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
    string_decoder: Option<syn::Path>,
    byte_array_decoder: Option<syn::Path>,
    encoding_decoder: Option<syn::Path>,
    enum_decoders: HashMap<String, syn::Path>,
    /// Map from callback element type name (e.g. `"Sample"`) to the decoder
    /// that builds an `impl Fn(T) + Send + Sync + 'static` closure from a
    /// `(callback: JObject, on_close: JObject)` pair.
    callback_decoders: HashMap<String, syn::Path>,
    /// Decoders for struct parameters passed across JNI as a plain `JObject`
    /// (e.g. a Kotlin `data class`). Keyed by the last-segment name of the
    /// parameter's type (e.g. `"HistoryConfig"`). The decoder must have
    /// signature `fn(&mut JNIEnv, &JObject) -> ZResult<T>`.
    struct_decoders: HashMap<String, syn::Path>,
    /// Per-function set of argument names that must be consumed (taken from
    /// the raw pointer via `Arc::from_raw`) instead of borrowed. Used for
    /// close/undeclare-style functions that invalidate their handle.
    consume_args: HashMap<String, HashSet<String>>,
    /// Return-type wrappers keyed by the last-segment name of `T` in
    /// `ZResult<T>`. Applies when `T` is a plain (non-`Vec`) type.
    return_wrappers: HashMap<String, ReturnWrapper>,
    /// Return-type wrappers keyed by the element type name of `Vec<T>` in
    /// `ZResult<Vec<T>>`.
    return_wrappers_vec: HashMap<String, ReturnWrapper>,
}

/// Describes how to render a `ZResult<T>` return value into a JNI-compatible
/// output value. Registered via [`Builder::return_wrapper`] /
/// [`Builder::return_wrapper_vec`].
#[derive(Clone)]
pub(crate) struct ReturnWrapper {
    jni_type: syn::Type,
    wrap_fn: syn::Path,
    default_expr: syn::Expr,
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
            string_decoder: None,
            byte_array_decoder: None,
            encoding_decoder: None,
            enum_decoders: HashMap::new(),
            callback_decoders: HashMap::new(),
            struct_decoders: HashMap::new(),
            consume_args: HashMap::new(),
            return_wrappers: HashMap::new(),
            return_wrappers_vec: HashMap::new(),
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

    /// Path of the function that decodes a `JString` into `String`, e.g.
    /// `"crate::utils::decode_string"`.
    pub fn string_decoder(mut self, path: impl AsRef<str>) -> Self {
        self.string_decoder =
            Some(syn::parse_str(path.as_ref()).expect("invalid string_decoder path"));
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

    /// Path of the function that decodes an `Encoding` from a `(jint id,
    /// &JString schema)` pair, e.g. `"crate::utils::decode_encoding"`.
    /// The generated JNI signature splits the single `Encoding` parameter
    /// into `<name>_id: jint` + `<name>_schema: JString`.
    pub fn encoding_decoder(mut self, path: impl AsRef<str>) -> Self {
        self.encoding_decoder =
            Some(syn::parse_str(path.as_ref()).expect("invalid encoding_decoder path"));
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

    /// Register a decoder for a struct parameter passed across JNI as a
    /// plain `JObject` (typically a Kotlin data class). `type_name` matches
    /// the last segment of the parameter's type path (e.g. `"HistoryConfig"`).
    /// The decoder must have signature
    /// `fn(&mut JNIEnv, &JObject) -> ZResult<T>`. Used both for the plain
    /// form (`HistoryConfig`) and the `Option<T>` form (nullable JObject).
    pub fn struct_decoder(
        mut self,
        type_name: impl Into<String>,
        decoder: impl AsRef<str>,
    ) -> Self {
        let path: syn::Path =
            syn::parse_str(decoder.as_ref()).expect("invalid struct_decoder path");
        self.struct_decoders.insert(type_name.into(), path);
        self
    }

    /// Register a decoder for an `impl Fn(T) + Send + Sync + 'static` callback
    /// parameter. `element_type_name` is the last path segment of `T`
    /// (e.g. `"Sample"`, `"Query"`, `"Reply"`). The decoder must have the
    /// signature
    /// `fn(&mut JNIEnv, JObject, JObject) -> ZResult<impl Fn(T) + Send + Sync + 'static>`.
    /// The generated JNI signature expands the single callback parameter into
    /// two JNI args: `<name>: JObject, <name>_on_close: JObject`.
    pub fn callback_decoder(
        mut self,
        element_type_name: impl Into<String>,
        decoder: impl AsRef<str>,
    ) -> Self {
        let path: syn::Path =
            syn::parse_str(decoder.as_ref()).expect("invalid callback_decoder path");
        self.callback_decoders.insert(element_type_name.into(), path);
        self
    }

    /// Register a return-type wrapper for `ZResult<T>` where `T`'s
    /// last-segment name equals `type_name`. `jni_type` is the generated
    /// `extern "C"` return type. `wrap_fn` must have signature
    /// `fn(&mut JNIEnv, T) -> ZResult<jni_type>`. `default_expr` is the value
    /// returned on error (before the exception is thrown on the JVM side).
    pub fn return_wrapper(
        mut self,
        type_name: impl Into<String>,
        jni_type: impl AsRef<str>,
        wrap_fn: impl AsRef<str>,
        default_expr: impl AsRef<str>,
    ) -> Self {
        self.return_wrappers
            .insert(type_name.into(), parse_return_wrapper(jni_type, wrap_fn, default_expr));
        self
    }

    /// Like [`Builder::return_wrapper`] but applies when `T` is `Vec<E>`
    /// with `E`'s last-segment name equal to `element_type_name`.
    pub fn return_wrapper_vec(
        mut self,
        element_type_name: impl Into<String>,
        jni_type: impl AsRef<str>,
        wrap_fn: impl AsRef<str>,
        default_expr: impl AsRef<str>,
    ) -> Self {
        self.return_wrappers_vec
            .insert(element_type_name.into(), parse_return_wrapper(jni_type, wrap_fn, default_expr));
        self
    }

    /// Mark a specific argument of a source function as consuming: the
    /// generated wrapper will take ownership of the raw pointer via
    /// `Arc::from_raw` (dropping the Arc at end of scope), rather than
    /// borrowing it through `OwnedObject::from_raw`. Applies to both
    /// `OpaqueRef` (`&T`) and `KeyExpr` arguments — for the latter, the
    /// string-fallback argument is omitted, leaving just the pointer.
    ///
    /// Typically used for `close_*` / `undeclare_*` functions that invalidate
    /// the handle.
    pub fn consume_arg(
        mut self,
        fn_name: impl Into<String>,
        arg_name: impl Into<String>,
    ) -> Self {
        self.consume_args
            .entry(fn_name.into())
            .or_default()
            .insert(arg_name.into());
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
        let empty_consume_set: HashSet<String> = HashSet::new();
        let consume_set = self
            .cfg
            .consume_args
            .get(&original_name)
            .unwrap_or(&empty_consume_set);

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
                    if consume_set.contains(&name.to_string()) {
                        prelude.push(quote! {
                            let #name = std::sync::Arc::from_raw(#ptr_ident);
                        });
                    } else {
                        prelude.push(quote! {
                            let #name = #owned_object::from_raw(#ptr_ident);
                        });
                    }
                    call_args.push(quote! { &#name });
                }
                ArgKind::KeyExpr => {
                    let ptr_ident = format_ident!("{}_ptr", name);
                    if consume_set.contains(&name.to_string()) {
                        // Consume path: the declared KeyExpr is required (no
                        // string fallback). Arc::from_raw decrements the
                        // refcount at end of scope, freeing the handle once
                        // no other references remain. A cloned inner KeyExpr
                        // is passed to the callee by value.
                        let arc_ident = format_ident!("__{}_arc", name);
                        jni_params.push(quote! {
                            #ptr_ident: *const zenoh::key_expr::KeyExpr<'static>
                        });
                        prelude.push(quote! {
                            let #arc_ident = std::sync::Arc::from_raw(#ptr_ident);
                            let #name = (*#arc_ident).clone();
                        });
                        call_args.push(quote! { #name });
                    } else {
                        let decoder = self
                            .cfg
                            .key_expr_decoder
                            .as_ref()
                            .expect("key_expr_decoder not configured");
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
                }
                ArgKind::String => {
                    let decoder = self
                        .cfg
                        .string_decoder
                        .as_ref()
                        .expect("string_decoder not configured");
                    jni_params.push(quote! { #name: jni::objects::JString });
                    prelude.push(quote! {
                        let #name = #decoder(&mut env, &#name)?;
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
                ArgKind::VecU8 => {
                    let decoder = self
                        .cfg
                        .byte_array_decoder
                        .as_ref()
                        .expect("byte_array_decoder not configured");
                    jni_params.push(quote! { #name: jni::objects::JByteArray });
                    prelude.push(quote! {
                        let #name = #decoder(&env, #name)?;
                    });
                    call_args.push(quote! { #name });
                }
                ArgKind::Callback(decoder) => {
                    let on_close_ident = format_ident!("{}_on_close", name);
                    jni_params.push(quote! { #name: jni::objects::JObject });
                    jni_params.push(quote! { #on_close_ident: jni::objects::JObject });
                    prelude.push(quote! {
                        let #name = #decoder(&mut env, #name, #on_close_ident)?;
                    });
                    call_args.push(quote! { #name });
                }
                ArgKind::Encoding => {
                    let decoder = self
                        .cfg
                        .encoding_decoder
                        .as_ref()
                        .expect("encoding_decoder not configured");
                    let id_ident = format_ident!("{}_id", name);
                    let schema_ident = format_ident!("{}_schema", name);
                    jni_params.push(quote! { #id_ident: jni::sys::jint });
                    jni_params.push(quote! { #schema_ident: jni::objects::JString });
                    prelude.push(quote! {
                        let #name = #decoder(&mut env, #id_ident, &#schema_ident)?;
                    });
                    call_args.push(quote! { #name });
                }
                ArgKind::OptionEncoding => {
                    let decoder = self
                        .cfg
                        .encoding_decoder
                        .as_ref()
                        .expect("encoding_decoder not configured");
                    let id_ident = format_ident!("{}_id", name);
                    let schema_ident = format_ident!("{}_schema", name);
                    jni_params.push(quote! { #id_ident: jni::sys::jint });
                    jni_params.push(quote! { #schema_ident: jni::objects::JString });
                    prelude.push(quote! {
                        let #name = Some(#decoder(&mut env, #id_ident, &#schema_ident)?);
                    });
                    call_args.push(quote! { #name });
                }
                ArgKind::OptionString => {
                    let decoder = self
                        .cfg
                        .string_decoder
                        .as_ref()
                        .expect("string_decoder not configured");
                    jni_params.push(quote! { #name: jni::objects::JString });
                    prelude.push(quote! {
                        let #name = if !#name.is_null() {
                            Some(#decoder(&mut env, &#name)?)
                        } else {
                            None
                        };
                    });
                    call_args.push(quote! { #name });
                }
                ArgKind::StructFromJObject(decoder) => {
                    jni_params.push(quote! { #name: jni::objects::JObject });
                    prelude.push(quote! {
                        let #name = #decoder(&mut env, &#name)?;
                    });
                    call_args.push(quote! { #name });
                }
                ArgKind::OptionStructFromJObject(decoder) => {
                    jni_params.push(quote! { #name: jni::objects::JObject });
                    prelude.push(quote! {
                        let #name = if !#name.is_null() {
                            Some(#decoder(&mut env, &#name)?)
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
                } else if let Some(wrapper) = self.lookup_return_wrapper(&inner) {
                    let ReturnWrapper {
                        jni_type,
                        wrap_fn,
                        default_expr,
                    } = wrapper;
                    (
                        quote! { #jni_type },
                        quote! { #wrap_fn(&mut env, __result) },
                        quote! { #default_expr },
                        quote! { #zresult<#jni_type> },
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

    fn lookup_return_wrapper(&self, ty: &syn::Type) -> Option<ReturnWrapper> {
        let syn::Type::Path(tp) = ty else { return None };
        let seg = tp.path.segments.last()?;
        let name = seg.ident.to_string();
        if name == "Vec" {
            let syn::PathArguments::AngleBracketed(args) = &seg.arguments else {
                return None;
            };
            let syn::GenericArgument::Type(elem) = args.args.first()? else {
                return None;
            };
            let elem_name = type_last_segment(elem)?;
            return self.cfg.return_wrappers_vec.get(&elem_name).cloned();
        }
        self.cfg.return_wrappers.get(&name).cloned()
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
            syn::Type::ImplTrait(it) => {
                if let Some(elem) = extract_fn_single_arg_type_name(&it.bounds) {
                    if let Some(decoder) = self.cfg.callback_decoders.get(&elem) {
                        return ArgKind::Callback(decoder.clone());
                    }
                }
                ArgKind::Unsupported
            }
            syn::Type::Path(tp) => {
                let Some(last) = tp.path.segments.last() else {
                    return ArgKind::Unsupported;
                };
                let name = last.ident.to_string();
                if name == "bool" {
                    return ArgKind::Bool;
                }
                if name == "String" {
                    return ArgKind::String;
                }
                if name == "KeyExpr" {
                    return ArgKind::KeyExpr;
                }
                if name == "Duration" {
                    return ArgKind::Duration;
                }
                if name == "Encoding" {
                    return ArgKind::Encoding;
                }
                if name == "Option" && is_option_of_vec_u8(last) {
                    return ArgKind::OptionVecU8;
                }
                if name == "Option" {
                    if let Some(inner) = option_inner_type_name(last) {
                        if inner == "String" {
                            return ArgKind::OptionString;
                        }
                        if inner == "Encoding" {
                            return ArgKind::OptionEncoding;
                        }
                        if let Some(decoder) = self.cfg.struct_decoders.get(&inner) {
                            return ArgKind::OptionStructFromJObject(decoder.clone());
                        }
                    }
                }
                if name == "Vec" && is_vec_of_u8(last) {
                    return ArgKind::VecU8;
                }
                if let Some(decoder) = self.cfg.enum_decoders.get(&name) {
                    return ArgKind::Enum(decoder.clone());
                }
                if let Some(decoder) = self.cfg.struct_decoders.get(&name) {
                    return ArgKind::StructFromJObject(decoder.clone());
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
    /// `String` → `JString` decoded via `string_decoder`.
    String,
    Enum(syn::Path),
    Bool,
    Duration,
    /// `Option<Vec<u8>>` → `JByteArray` decoded via `byte_array_decoder`.
    OptionVecU8,
    /// `Vec<u8>` → `JByteArray` decoded via `byte_array_decoder`.
    VecU8,
    /// `Encoding` → `(jint id, JString schema)` pair via `encoding_decoder`.
    Encoding,
    /// `Option<Encoding>` → `(jint id, JString schema)` pair via `encoding_decoder`,
    /// wrapped in `Some(_)`. Semantic gating on payload presence is the
    /// callee's responsibility.
    OptionEncoding,
    /// `Option<String>` → nullable `JString`.
    OptionString,
    /// Struct type registered via `struct_decoder` → single `JObject` arg
    /// decoded via the registered decoder.
    StructFromJObject(syn::Path),
    /// `Option<T>` where `T` is registered via `struct_decoder` → nullable
    /// `JObject`, `None` when the JObject is null.
    OptionStructFromJObject(syn::Path),
    /// `impl Fn(T) + Send + Sync + 'static` → `(JObject callback, JObject on_close)`
    /// pair decoded via a callback decoder registered for `T`.
    Callback(syn::Path),
    Unsupported,
}

fn parse_return_wrapper(
    jni_type: impl AsRef<str>,
    wrap_fn: impl AsRef<str>,
    default_expr: impl AsRef<str>,
) -> ReturnWrapper {
    ReturnWrapper {
        jni_type: syn::parse_str(jni_type.as_ref()).expect("invalid return wrapper jni_type"),
        wrap_fn: syn::parse_str(wrap_fn.as_ref()).expect("invalid return wrapper wrap_fn path"),
        default_expr: syn::parse_str(default_expr.as_ref())
            .expect("invalid return wrapper default_expr"),
    }
}

fn type_last_segment(ty: &syn::Type) -> Option<String> {
    let syn::Type::Path(tp) = ty else { return None };
    tp.path.segments.last().map(|s| s.ident.to_string())
}

/// Look through the trait bounds of an `impl Fn(T) + ...` for a `Fn`-family
/// trait and return the last-segment name of its single argument type `T`.
fn extract_fn_single_arg_type_name(
    bounds: &syn::punctuated::Punctuated<syn::TypeParamBound, syn::Token![+]>,
) -> Option<String> {
    for bound in bounds {
        let syn::TypeParamBound::Trait(tb) = bound else { continue };
        let seg = tb.path.segments.last()?;
        if !matches!(seg.ident.to_string().as_str(), "Fn" | "FnMut" | "FnOnce") {
            continue;
        }
        let syn::PathArguments::Parenthesized(p) = &seg.arguments else { continue };
        let first = p.inputs.first()?;
        return type_last_segment(first);
    }
    None
}

/// Return the last-segment name of the single generic argument of an
/// `Option<...>` path segment, if any (e.g. `Option<String>` → `Some("String")`).
fn option_inner_type_name(seg: &syn::PathSegment) -> Option<String> {
    let syn::PathArguments::AngleBracketed(args) = &seg.arguments else {
        return None;
    };
    let syn::GenericArgument::Type(inner) = args.args.first()? else {
        return None;
    };
    type_last_segment(inner)
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
    is_vec_of_u8(inner_seg)
}

/// Check whether a `Vec<...>` path segment has element type `u8`.
fn is_vec_of_u8(seg: &syn::PathSegment) -> bool {
    let syn::PathArguments::AngleBracketed(args) = &seg.arguments else {
        return false;
    };
    let Some(syn::GenericArgument::Type(elem)) = args.args.first() else {
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
