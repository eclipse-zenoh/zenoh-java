//! Universal function-processing converter.
//!
//! [`FunctionsConverter`] consumes `#[prebindgen]` `syn::ItemFn`s from the
//! source iterator, looks up each parameter / return type in the shared
//! [`TypeRegistry`], builds the wrapper signature from plain config
//! (extra leading params, attrs, ABI, unsafety, name mangler), and
//! delegates body construction to a [`BodyStrategy`].
//!
//! ## Mapping to today's pipelines
//!
//! **JNI/Kotlin** (the live pipeline in `zenoh-jni/build.rs`):
//! * [`NameMangler::CamelPrefixSuffix`] with `Java_<class>_…` prefix and
//!   `ViaJNI` suffix.
//! * extra leading params: `mut env: jni::JNIEnv, _class: jni::objects::JClass`.
//! * `extern_abi: extern "C"`, `unsafety: true`,
//!   attrs `#[no_mangle] #[allow(non_snake_case, …)]`.
//! * body strategy: `jni::body_strategy::JniTryClosureBody`.
//!
//! **C/cbindgen** (validation target — equivalent to today's
//! `prebindgen::batching::FfiConverter`):
//! * [`NameMangler::Identity`].
//! * no extra leading params.
//! * `extern_abi: extern "C"`, `unsafety: true`,
//!   attrs `#[no_mangle] #[allow(clippy::missing_safety_doc)]`.
//! * body strategy: [`PassThroughBody`] (would need transmute-style arg
//!   conversion via custom `decode` closures or a future
//!   `TransmuteBodyStrategy`).

use std::collections::VecDeque;

use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};

use prebindgen::SourceLocation;

use crate::core::name_mangler::NameMangler;
use crate::core::return_encode::ReturnEncode;
use crate::core::type_binding::TypeBinding;
use crate::core::type_registry::TypeRegistry;
use crate::util::is_unit;

/// Context passed to [`BodyStrategy::build_body`]. Exposes everything the
/// strategy needs to assemble the function body.
pub struct BodyContext<'a> {
    /// Decoded-arg `let` bindings, in source-parameter order.
    /// Each entry looks like `let foo = <decode_expr>;`.
    pub prelude: &'a [TokenStream],
    /// Tokens of the call expression
    /// `<source_module>::<orig_ident>(<call_args>)`.
    pub call_expr: TokenStream,
    /// Wire-side return type produced by the wrapper. `None` for unit
    /// returns. Strategies that build a try-closure use this as the
    /// closure's `Ok` type.
    pub wire_return: Option<&'a syn::Type>,
    /// Return-direction encode info, if the return has a registered
    /// [`TypeBinding`] with `encode` set.
    pub return_encode: Option<&'a ReturnEncode>,
    /// Default expression for the error path (paired with `return_encode`).
    pub return_default: Option<&'a syn::Expr>,
    /// Original function ident (for diagnostics).
    pub orig_ident: &'a syn::Ident,
    /// Source location (for diagnostics).
    pub loc: &'a SourceLocation,
}

/// Strategy for assembling the wrapper function body.
pub trait BodyStrategy {
    fn build_body(&self, ctx: BodyContext) -> TokenStream;
}

/// Body strategy that emits the call expression directly, applying any
/// configured `ReturnEncode` inline. Used by pass-through C-style FFI.
pub struct PassThroughBody;

impl BodyStrategy for PassThroughBody {
    fn build_body(&self, ctx: BodyContext) -> TokenStream {
        let prelude = ctx.prelude;
        let call = &ctx.call_expr;
        match ctx.return_encode {
            None => quote! { { #(#prelude)* #call } },
            Some(ReturnEncode::Wrapper(p)) => quote! {
                {
                    #(#prelude)*
                    let __result = #call;
                    #p(__result)
                }
            },
            Some(ReturnEncode::ArcIntoRaw) => quote! {
                {
                    #(#prelude)*
                    let __result = #call;
                    std::sync::Arc::into_raw(std::sync::Arc::new(__result))
                }
            },
        }
    }
}

/// Builder for [`FunctionsConverter`].
pub struct FunctionsBuilder {
    pub(crate) source_module: syn::Path,
    pub(crate) name_mangler: NameMangler,
    pub(crate) extra_leading_params: TokenStream,
    pub(crate) extra_attrs: Vec<syn::Attribute>,
    pub(crate) extern_abi: Option<syn::Abi>,
    pub(crate) unsafety: bool,
    pub(crate) types: TypeRegistry,
    pub(crate) body_strategy: Box<dyn BodyStrategy>,
}

impl FunctionsBuilder {
    fn new(body_strategy: Box<dyn BodyStrategy>) -> Self {
        Self {
            source_module: syn::parse_str("crate").unwrap(),
            name_mangler: NameMangler::Identity,
            extra_leading_params: TokenStream::new(),
            extra_attrs: Vec::new(),
            extern_abi: None,
            unsafety: false,
            types: TypeRegistry::new(),
            body_strategy,
        }
    }

    /// Module path that contains the original `#[prebindgen]` functions
    /// being wrapped. The wrapper body calls `<source_module>::<orig>(…)`.
    pub fn source_module(mut self, path: impl AsRef<str>) -> Self {
        self.source_module = syn::parse_str(path.as_ref()).expect("invalid source_module path");
        self
    }

    pub fn name_mangler(mut self, mangler: NameMangler) -> Self {
        self.name_mangler = mangler;
        self
    }

    /// Extra parameters prepended before the wrapped function's user
    /// parameters. Pass an empty `TokenStream` for none.
    pub fn extra_leading_params(mut self, params: TokenStream) -> Self {
        self.extra_leading_params = params;
        self
    }

    /// Extra outer attributes applied to the wrapper function.
    pub fn extra_attrs(mut self, attrs: Vec<syn::Attribute>) -> Self {
        self.extra_attrs = attrs;
        self
    }

    /// Function ABI (typically `extern "C"`).
    pub fn extern_abi(mut self, abi: syn::Abi) -> Self {
        self.extern_abi = Some(abi);
        self
    }

    /// Whether the wrapper is `unsafe fn`.
    pub fn unsafety(mut self, unsafety: bool) -> Self {
        self.unsafety = unsafety;
        self
    }

    /// Register or replace a [`TypeBinding`] by name.
    pub fn type_binding(mut self, binding: TypeBinding) -> Self {
        self.types.insert_raw(binding.name().to_string(), binding);
        self
    }

    /// Merge a reusable [`TypeRegistry`] into the registry.
    pub fn type_registry(mut self, registry: TypeRegistry) -> Self {
        self.types.extend_from(registry);
        self
    }

    pub fn build(self) -> FunctionsConverter {
        FunctionsConverter {
            cfg: self,
            pending: VecDeque::new(),
            buffered: false,
        }
    }
}

pub struct FunctionsConverter {
    pub(crate) cfg: FunctionsBuilder,
    pending: VecDeque<(syn::Item, SourceLocation)>,
    buffered: bool,
}

impl FunctionsConverter {
    pub fn builder<B: BodyStrategy + 'static>(body_strategy: B) -> FunctionsBuilder {
        FunctionsBuilder::new(Box::new(body_strategy))
    }

    pub fn call<I>(&mut self, iter: &mut I) -> Option<(syn::Item, SourceLocation)>
    where
        I: Iterator<Item = (syn::Item, SourceLocation)>,
    {
        if !self.buffered {
            self.buffered = true;
            for (item, loc) in iter.by_ref() {
                let converted = self.convert(item, &loc);
                self.pending.push_back((converted, loc));
            }
        }
        self.pending.pop_front()
    }

    pub fn as_closure<'a, I>(
        &'a mut self,
    ) -> impl FnMut(&mut I) -> Option<(syn::Item, SourceLocation)> + 'a
    where
        I: Iterator<Item = (syn::Item, SourceLocation)>,
    {
        move |iter| self.call(iter)
    }

    /// Borrow the populated [`TypeRegistry`]. Useful for downstream
    /// consumers (e.g. a Kotlin interface generator).
    pub fn type_registry(&self) -> &TypeRegistry {
        &self.cfg.types
    }

    fn convert(&mut self, item: syn::Item, loc: &SourceLocation) -> syn::Item {
        match item {
            syn::Item::Fn(func) => syn::Item::Fn(self.convert_fn(func, loc)),
            other => panic!(
                "FunctionsConverter received a non-fn item at {loc}: {}",
                other.to_token_stream()
            ),
        }
    }

    fn convert_fn(&mut self, func: syn::ItemFn, loc: &SourceLocation) -> syn::ItemFn {
        let original_ident = func.sig.ident.clone();
        let wrapper_ident = self.cfg.name_mangler.apply(&original_ident);
        let source_module = self.cfg.source_module.clone();

        let mut prelude: Vec<TokenStream> = Vec::new();
        let mut wire_params: Vec<TokenStream> = Vec::new();
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

            self.emit_arg(name, ty, loc, &mut prelude, &mut wire_params, &mut call_args);
        }

        let (wire_return_ty, return_encode, return_default): (
            Option<syn::Type>,
            Option<ReturnEncode>,
            Option<syn::Expr>,
        ) = match &func.sig.output {
            syn::ReturnType::Default => (None, None, None),
            syn::ReturnType::Type(_, ty) => {
                if is_unit(ty) {
                    (None, None, None)
                } else {
                    let key = ty.to_token_stream().to_string();
                    let binding = self.cfg.types.types.get(&key).unwrap_or_else(|| {
                        panic!(
                            "unsupported return type `{}` for `{}` at {loc}: \
                             register a TypeBinding keyed `{}`",
                            ty.to_token_stream(),
                            original_ident,
                            key
                        )
                    });
                    let wire = binding.wire_type().clone();
                    let encode = binding.encode().cloned();
                    let default = binding.default_expr().cloned();
                    // Treat a `()` wire type as the unit case so body
                    // strategies have a single canonical "no return" shape.
                    if is_unit(&wire) {
                        (None, encode, default)
                    } else {
                        (Some(wire), encode, default)
                    }
                }
            }
        };

        let call_expr = quote! { #source_module::#original_ident( #(#call_args),* ) };
        let body = self.cfg.body_strategy.build_body(BodyContext {
            prelude: &prelude,
            call_expr,
            wire_return: wire_return_ty.as_ref(),
            return_encode: return_encode.as_ref(),
            return_default: return_default.as_ref(),
            orig_ident: &original_ident,
            loc,
        });

        let wire_return_tokens: TokenStream = match &wire_return_ty {
            None => quote! { () },
            Some(t) => quote! { #t },
        };

        let attrs = &self.cfg.extra_attrs;
        let abi = self.cfg.extern_abi.as_ref();
        let unsafe_kw = if self.cfg.unsafety {
            quote! { unsafe }
        } else {
            quote! {}
        };
        let extra_leading = &self.cfg.extra_leading_params;
        let leading_separator: TokenStream = if extra_leading.is_empty() || wire_params.is_empty() {
            TokenStream::new()
        } else {
            quote! { , }
        };

        let abi_tokens = match abi {
            Some(a) => quote! { #a },
            None => TokenStream::new(),
        };

        let tokens = quote! {
            #(#attrs)*
            pub #unsafe_kw #abi_tokens fn #wrapper_ident(
                #extra_leading #leading_separator #(#wire_params),*
            ) -> #wire_return_tokens #body
        };

        syn::parse2(tokens).expect("generated wrapper must parse")
    }

    fn emit_arg(
        &self,
        name: &syn::Ident,
        ty: &syn::Type,
        loc: &SourceLocation,
        prelude: &mut Vec<TokenStream>,
        wire_params: &mut Vec<TokenStream>,
        call_args: &mut Vec<TokenStream>,
    ) {
        let key = ty.to_token_stream().to_string();
        let binding = self.cfg.types.types.get(&key).unwrap_or_else(|| {
            panic!(
                "unsupported parameter type `{}` for `{}` at {loc}: \
                 register a TypeBinding keyed `{}`",
                ty.to_token_stream(),
                name,
                key
            )
        });

        let pat = if binding.is_pointer() {
            format_ident!("{}_ptr", name)
        } else {
            name.clone()
        };
        let wire_ty = binding.wire_type();
        wire_params.push(quote! { #pat: #wire_ty });

        let decoded = binding
            .call_decode(&pat)
            .unwrap_or_else(|| panic!("TypeBinding `{}` has no decode at {loc}", key));
        prelude.push(quote! { let #name = #decoded; });

        if binding.is_borrow() {
            call_args.push(quote! { &#name });
        } else {
            call_args.push(quote! { #name });
        }
    }
}
