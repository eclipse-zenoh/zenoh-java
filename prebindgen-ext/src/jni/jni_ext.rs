//! JNI implementation of [`PrebindgenExt`].
//!
//! Provides the universal JNI patterns:
//! * **Wrapper signatures**: input converter is
//!   `fn(env: &mut JNIEnv, v: <wire>) -> ZResult<<rust>>`; output converter
//!   is `fn(env: &mut JNIEnv, v: &<rust>) -> ZResult<<wire>>`.
//! * **`on_function`**: emits a JNI `extern "C"` wrapper that delegates each
//!   parameter conversion to the auto-generated `<rust>_to_<wire>_<hash>`
//!   converter, calls the original `#[prebindgen]` fn, and routes errors
//!   through a configurable `throw_exception!` macro.
//! * **Primitive types**: `bool`, `i64`, `f64`, `Duration`, `String`,
//!   `Vec<u8>` rank-0 input/output bodies.
//! * **Wildcard wrappers**: `Option<_>` (input + output, including
//!   primitive boxing), `ZResult<_>` (output only), `impl Fn(_..)` rank-1/2/3
//!   input (callback wrappers).
//! * **Structs/enums**: rank-0 input/output bodies are built from the
//!   `Registry`'s `structs` / `enums` maps — fields and variants get
//!   converted via the same auto-generated converter names.
//!
//! Crate-specific match arms (zenoh's `legacy_bindings` rows like
//! `CongestionControl`, manual callback overrides, opaque borrows, etc.)
//! belong in a thin wrapper trait impl in the consuming crate's `build.rs`,
//! NOT in this module — keeps `prebindgen-ext` reusable for any JNI/Kotlin
//! project.

use std::path::PathBuf;

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, ToTokens};

use crate::core::niches::Niches;
use crate::core::prebindgen_ext::{ConverterImpl, PrebindgenExt};
use crate::core::registry::{extract_fn_trait_args, Registry, TypeKey};
use crate::jni::wire_access::jni_field_access;
use crate::util::snake_to_camel;

/// JNI back-end. Configure paths in the Rust crate (zresult, throw macro,
/// source module the original fns live in) and the JNI/Kotlin classpath
/// (java class prefix, callback Kotlin package + output dir).
#[derive(Clone)]
pub struct JniExt {
    /// Module path the original `#[prebindgen]` fns live under (e.g.
    /// `zenoh_flat`). The wrapper body calls `<source_module>::<fn>(args)`.
    pub source_module: syn::Path,
    /// `Result` type used by emitted converter and wrapper signatures
    /// (e.g. `crate::errors::ZResult`).
    pub zresult: syn::Path,
    /// Path to the `throw_exception` macro (or fn) used by `on_function`'s
    /// error path. Must be invoked as `<path>!(env, err)`.
    pub throw_macro: syn::Path,
    /// Java class path prefix for auto-generated struct encoders, slash-
    /// separated (e.g. `io/zenoh/jni`). Empty = no prefix.
    pub java_class_prefix: String,
    /// JNI native-class name, used by [`Self::on_function`] when mangling
    /// fn idents (e.g. `Java_io_zenoh_jni_JNINative_<fn>ViaJNI`).
    pub jni_class_path: String,
    /// Suffix appended to the wrapped fn name (e.g. `ViaJNI`).
    pub jni_method_suffix: String,
    /// Kotlin package callback fun-interfaces are emitted into.
    pub kotlin_callback_package: String,
    /// On-disk directory the per-callback `.kt` files are written to.
    pub kotlin_callback_dir: PathBuf,
}

impl JniExt {
    /// Convenience constructor with sensible defaults; the paths still need
    /// to be set explicitly via the field-mutation builder methods.
    pub fn new() -> Self {
        Self {
            source_module: syn::parse_str("crate").unwrap(),
            zresult: syn::parse_str("crate::errors::ZResult").unwrap(),
            throw_macro: syn::parse_str("crate::throw_exception").unwrap(),
            java_class_prefix: String::new(),
            jni_class_path: "Java".to_string(),
            jni_method_suffix: String::new(),
            kotlin_callback_package: String::new(),
            kotlin_callback_dir: PathBuf::new(),
        }
    }
    pub fn source_module(mut self, p: impl AsRef<str>) -> Self {
        self.source_module = syn::parse_str(p.as_ref()).expect("invalid source_module path");
        self
    }
    pub fn zresult(mut self, p: impl AsRef<str>) -> Self {
        self.zresult = syn::parse_str(p.as_ref()).expect("invalid zresult path");
        self
    }
    pub fn throw_macro(mut self, p: impl AsRef<str>) -> Self {
        self.throw_macro = syn::parse_str(p.as_ref()).expect("invalid throw_macro path");
        self
    }
    pub fn java_class_prefix(mut self, p: impl Into<String>) -> Self {
        self.java_class_prefix = p.into().trim_matches('/').to_string();
        self
    }
    pub fn jni_class_path(mut self, p: impl Into<String>) -> Self {
        self.jni_class_path = p.into();
        self
    }
    pub fn jni_method_suffix(mut self, s: impl Into<String>) -> Self {
        self.jni_method_suffix = s.into();
        self
    }
    pub fn kotlin_callback_package(mut self, p: impl Into<String>) -> Self {
        self.kotlin_callback_package = p.into();
        self
    }
    pub fn kotlin_callback_dir(mut self, d: impl Into<PathBuf>) -> Self {
        self.kotlin_callback_dir = d.into();
        self
    }
}

impl Default for JniExt {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────────────────────────────
// Inherent helpers — wrapper builders (used by both PrebindgenExt impl
// and consuming-crate wrapper exts like ZenohJniExt).
// ──────────────────────────────────────────────────────────────────────

impl JniExt {
    /// Build the standard JNI input-converter `fn`. Body assumes in-scope
    /// `env: &mut JNIEnv` and `v: &<wire>` (or `v: <wire>` for raw-pointer
    /// wires); produces a value of `rust`. Returned function has its name
    /// already set per the JNI plugin's naming convention.
    pub fn input_wrapper(
        &self,
        rust: &syn::Type,
        wire: &syn::Type,
        body: &syn::Expr,
    ) -> syn::ItemFn {
        let name = input_name(rust, wire);
        let zresult = &self.zresult;
        let rust_with_lifetime = annotate_borrow_with_lifetime(rust, "env");
        let wire_with_lifetime = annotate_jobject_with_lifetime(wire, "v");
        if matches!(wire, syn::Type::Ptr(_)) {
            syn::parse_quote!(
                #[allow(non_snake_case, unused_mut, unused_variables, unused_braces, dead_code)]
                pub(crate) unsafe fn #name<'env>(env: &mut jni::JNIEnv<'env>, v: #wire) -> #zresult<#rust_with_lifetime> {
                    Ok(#body)
                }
            )
        } else {
            syn::parse_quote!(
                #[allow(non_snake_case, unused_mut, unused_variables, unused_braces, dead_code)]
                pub(crate) unsafe fn #name<'env, 'v>(env: &mut jni::JNIEnv<'env>, v: &#wire_with_lifetime) -> #zresult<#rust_with_lifetime> {
                    Ok(#body)
                }
            )
        }
    }

    /// Build the standard JNI output-converter `fn`. Body assumes in-scope
    /// `env: &mut JNIEnv` and `v: <rust>` (by value — handles like
    /// `Subscriber<()>` aren't `Clone`, so callers move into the converter).
    pub fn output_wrapper(
        &self,
        rust: &syn::Type,
        wire: &syn::Type,
        body: &syn::Expr,
    ) -> syn::ItemFn {
        let name = output_name(rust, wire);
        let zresult = &self.zresult;
        let wire_with_lifetime = annotate_jobject_with_lifetime(wire, "a");
        syn::parse_quote!(
            #[allow(non_snake_case, unused_mut, unused_variables, unused_braces, dead_code)]
            pub(crate) unsafe fn #name<'a>(env: &mut jni::JNIEnv<'a>, v: #rust) -> #zresult<#wire_with_lifetime> {
                Ok(#body)
            }
        )
    }

    /// Universal "opaque Arc-handle as `jlong`" pair — input side.
    ///
    /// Use for any Rust type whose lifecycle is owned by the Java side:
    /// Java holds the master `Arc` as a `Long`, calls Rust passing the
    /// pointer, and explicitly destroys via a separate `dropXxxViaJNI`
    /// JNI fn that does one matching `Arc::from_raw(v)` drop.
    ///
    /// **Convention** (single rule for both input and output):
    /// * Wire: `jni::sys::jlong` — the same width JNI hands across
    ///   the boundary on every platform (`*const T` would mismatch
    ///   on 32-bit, where ptr size is 4 but jlong is 8).
    /// * Output: `Arc::into_raw(Arc::new(v)) as i64` — wrap once, leak
    ///   the pointer to Java. Refcount = 1 sitting in the leaked state.
    /// * Input: `unsafe { (*( *v as *const T)).clone() }` — non-Arc
    ///   read. The pointer is bit-cast, dereferenced, and the inner
    ///   value cloned. The outer `Arc<T>` refcount is **never** touched
    ///   by per-call decoding, so Java may pass the same handle as
    ///   many times as it likes.
    /// * Niche: `0i64` / `*v == 0` — `Arc::into_raw` never returns 0,
    ///   so `Option<T>` automatically synthesises `0` = `None`,
    ///   matching the legacy "null pointer" ABI for nullable handles.
    ///
    /// Requires `T: Clone` — almost always satisfied for opaque
    /// handles, since they typically wrap an internal `Arc<Inner>`
    /// whose `Clone` just bumps the inner refcount.
    pub fn opaque_arc_input(&self, ty: &syn::Type) -> ConverterImpl {
        let wire: syn::Type = syn::parse_quote!(jni::sys::jlong);
        let body: syn::Expr = syn::parse_quote!(unsafe {
            let raw = *v as *const #ty;
            (*raw).clone()
        });
        ConverterImpl {
            function: self.input_wrapper(ty, &wire, &body),
            destination: wire,
            niches: Niches::one(
                syn::parse_quote!(0i64),
                syn::parse_quote!(*v == 0),
            ),
        }
    }

    /// Output side of [`Self::opaque_arc_input`] — see that method's
    /// docs for the full convention.
    pub fn opaque_arc_output(&self, ty: &syn::Type) -> ConverterImpl {
        let wire: syn::Type = syn::parse_quote!(jni::sys::jlong);
        let body: syn::Expr = syn::parse_quote!(
            std::sync::Arc::into_raw(std::sync::Arc::new(v)) as i64
        );
        ConverterImpl {
            function: self.output_wrapper(ty, &wire, &body),
            destination: wire,
            niches: Niches::one(
                syn::parse_quote!(0i64),
                syn::parse_quote!(*v == 0),
            ),
        }
    }
}

// ──────────────────────────────────────────────────────────────────────
// PrebindgenExt impl
// ──────────────────────────────────────────────────────────────────────

impl PrebindgenExt for JniExt {
    // ── Item methods ─────────────────────────────────────────────────

    fn on_function(&self, f: &syn::ItemFn, registry: &Registry) -> TokenStream {
        emit_jni_function_wrapper(self, f, registry)
    }

    fn on_struct(&self, _s: &syn::ItemStruct, _registry: &Registry) -> TokenStream {
        // Struct converter bodies are emitted by the resolver via
        // on_input_type_rank_0 / on_output_type_rank_0 below; no separate
        // per-struct item is needed.
        TokenStream::new()
    }

    fn on_enum(&self, _e: &syn::ItemEnum, _registry: &Registry) -> TokenStream {
        TokenStream::new()
    }

    fn on_const(&self, c: &syn::ItemConst, _registry: &Registry) -> TokenStream {
        c.to_token_stream()
    }

    // ── Input converters ─────────────────────────────────────────────

    fn on_input_type_rank_0(
        &self,
        ty: &syn::Type,
        registry: &Registry,
    ) -> Option<ConverterImpl> {
        if let Some((wire, body)) = primitive_input(ty) {
            let niches = default_niches_for_wire(&wire);
            return Some(ConverterImpl {
                function: self.input_wrapper(ty, &wire, &body),
                destination: wire,
                niches,
            });
        }
        if let Some(name) = bare_path_ident(ty) {
            if let Some((s, _)) = registry.structs.get(&name) {
                let (wire, body) = struct_input_body(self, s, registry)?;
                let niches = default_niches_for_wire(&wire);
                return Some(ConverterImpl {
                    function: self.input_wrapper(ty, &wire, &body),
                    destination: wire,
                    niches,
                });
            }
            // Bare-ident enum: leave to the consuming crate to override
            // (today's CongestionControl etc. fall here — caller's wrapper
            // ext returns Some in its own on_input_type_rank_0).
        }
        None
    }

    fn on_input_type_rank_1(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        registry: &Registry,
    ) -> Option<ConverterImpl> {
        // `& _` borrow: a free-fn converter can't return `&T` (no borrow
        // source), so we *share* T's resolved converter — `&T`'s entry
        // points at the same `ItemFn`. The fn returns owned `T`; the
        // call site in `emit_jni_function_wrapper` adds `&decoded` when
        // the original param was `&T`. write.rs's dedup-by-name keeps
        // the function emitted exactly once.
        //
        // This handler exists to make the wildcard-substitution machinery
        // fire: it returns subs=[t1] (via the resolver), so propagation
        // marks T as required transitively from `&T`.
        if pat_match(pat, "& _") {
            let inner = registry.input_entry(t1)?;
            return Some(ConverterImpl {
                destination: inner.destination.clone(),
                function: inner.function.clone(),
                niches: inner.niches.clone(),
            });
        }
        if pat_match(pat, "Option < _ >") {
            let outer_ty: syn::Type = syn::parse_quote!(Option<#t1>);
            let (wire, body, niches) = option_input(t1, registry)?;
            return Some(ConverterImpl {
                function: self.input_wrapper(&outer_ty, &wire, &body),
                destination: wire,
                niches,
            });
        }
        if let Some(args) = extract_fn_trait_args(pat) {
            if args.len() == 1 {
                let arg_tys = std::slice::from_ref(t1);
                let outer_ty = build_fn_type(arg_tys);
                let (wire, body) = callback_input(self, arg_tys, registry)?;
                let niches = default_niches_for_wire(&wire);
                return Some(ConverterImpl {
                    function: self.input_wrapper(&outer_ty, &wire, &body),
                    destination: wire,
                    niches,
                });
            }
        }
        None
    }

    fn on_input_type_rank_2(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        t2: &syn::Type,
        registry: &Registry,
    ) -> Option<ConverterImpl> {
        if let Some(args) = extract_fn_trait_args(pat) {
            if args.len() == 2 {
                let arg_tys = [t1.clone(), t2.clone()];
                let outer_ty = build_fn_type(&arg_tys);
                let (wire, body) = callback_input(self, &arg_tys, registry)?;
                let niches = default_niches_for_wire(&wire);
                return Some(ConverterImpl {
                    function: self.input_wrapper(&outer_ty, &wire, &body),
                    destination: wire,
                    niches,
                });
            }
        }
        None
    }

    fn on_input_type_rank_3(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        t2: &syn::Type,
        t3: &syn::Type,
        registry: &Registry,
    ) -> Option<ConverterImpl> {
        if let Some(args) = extract_fn_trait_args(pat) {
            if args.len() == 3 {
                let arg_tys = [t1.clone(), t2.clone(), t3.clone()];
                let outer_ty = build_fn_type(&arg_tys);
                let (wire, body) = callback_input(self, &arg_tys, registry)?;
                let niches = default_niches_for_wire(&wire);
                return Some(ConverterImpl {
                    function: self.input_wrapper(&outer_ty, &wire, &body),
                    destination: wire,
                    niches,
                });
            }
        }
        None
    }

    // ── Output converters ────────────────────────────────────────────

    fn on_output_type_rank_0(
        &self,
        ty: &syn::Type,
        registry: &Registry,
    ) -> Option<ConverterImpl> {
        // `()` — identity converter so `fn foo()` and `fn foo() -> ()`
        // funnel through the same uniform output path as everything else.
        // Wire is `()`. Body just returns `v`.
        if pat_match(ty, "()") {
            let wire: syn::Type = syn::parse_quote!(());
            let body: syn::Expr = syn::parse_quote!(v);
            return Some(ConverterImpl {
                function: self.output_wrapper(ty, &wire, &body),
                destination: wire,
                niches: Niches::empty(),
            });
        }
        if let Some((wire, body)) = primitive_output(ty) {
            let niches = default_niches_for_wire(&wire);
            return Some(ConverterImpl {
                function: self.output_wrapper(ty, &wire, &body),
                destination: wire,
                niches,
            });
        }
        if let Some(name) = bare_path_ident(ty) {
            if let Some((s, _)) = registry.structs.get(&name) {
                let (wire, body) = struct_output_body(self, s, registry)?;
                let niches = default_niches_for_wire(&wire);
                return Some(ConverterImpl {
                    function: self.output_wrapper(ty, &wire, &body),
                    destination: wire,
                    niches,
                });
            }
        }
        None
    }

    fn on_output_type_rank_1(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        registry: &Registry,
    ) -> Option<ConverterImpl> {
        // `ZResult<_>` — unwrap via `?`, then delegate to inner T's output
        // converter. Wire is T's wire; the body calls T's converter on the
        // unwrapped value. Subs=[T] is recorded by the resolver so
        // propagation marks T required.
        //
        // Note: the source-side type the user wrote is the bare-name
        // `ZResult` (matching the prebindgen scan key). The wrapper takes
        // `v: <zresult-path>< T >` so it resolves at compile time in the
        // host crate even though `ZResult` isn't in scope at the include
        // site — we use the configured `self.zresult` path instead of
        // bare `ZResult`.
        if pat_match(pat, "ZResult < _ >") {
            let inner = registry.output_entry(t1)?;
            let inner_wire = inner.destination.clone();
            let inner_conv = inner.function.sig.ident.clone();
            // `ZResult<T>` propagates `Err` via `?` and emits inner's wire
            // on `Ok`. The success path produces exactly the same set of
            // wire values as the inner converter, so the wrapper exposes
            // inner's niches verbatim — an enclosing `Option<ZResult<T>>`
            // can carve from them just as if it wrapped `T` directly.
            let inherited_niches = inner.niches.clone();
            let zresult_path = &self.zresult;
            let outer_ty: syn::Type = syn::parse_quote!(#zresult_path<#t1>);
            let body: syn::Expr = syn::parse_quote!({
                let __inner = v?;
                #inner_conv(env, __inner)?
            });
            return Some(ConverterImpl {
                function: self.output_wrapper(&outer_ty, &inner_wire, &body),
                destination: inner_wire,
                niches: inherited_niches,
            });
        }
        if pat_match(pat, "Option < _ >") {
            let outer_ty: syn::Type = syn::parse_quote!(Option<#t1>);
            let (wire, body, niches) = option_output(t1, registry)?;
            return Some(ConverterImpl {
                function: self.output_wrapper(&outer_ty, &wire, &body),
                destination: wire,
                niches,
            });
        }
        None
    }

    fn on_output_type_rank_2(
        &self,
        _pat: &syn::Type,
        _t1: &syn::Type,
        _t2: &syn::Type,
        _registry: &Registry,
    ) -> Option<ConverterImpl> {
        None
    }

    fn on_output_type_rank_3(
        &self,
        _pat: &syn::Type,
        _t1: &syn::Type,
        _t2: &syn::Type,
        _t3: &syn::Type,
        _registry: &Registry,
    ) -> Option<ConverterImpl> {
        None
    }
}

// ──────────────────────────────────────────────────────────────────────
// Function-wrapper emission (JNI extern "C")
// ──────────────────────────────────────────────────────────────────────

fn emit_jni_function_wrapper(ext: &JniExt, f: &syn::ItemFn, registry: &Registry) -> TokenStream {
    let original_ident = &f.sig.ident;
    let wrapper_ident = mangle_jni_name(ext, original_ident);
    let source_module = &ext.source_module;
    let throw = &ext.throw_macro;

    let mut wire_params: Vec<TokenStream> = Vec::new();
    let mut prelude: Vec<TokenStream> = Vec::new();
    let mut call_args: Vec<TokenStream> = Vec::new();

    // Input parameters: look up converter for the param type AS WRITTEN.
    // No strip — a `&T` param looks up `&T`'s entry (which the `& _`
    // rank-1 handler resolved by sharing `T`'s function). Call site adds
    // `&decoded` only for `&T`-shaped originals; that's a Rust call-
    // convention concern, not a converter concern.
    for input in &f.sig.inputs {
        let syn::FnArg::Typed(pt) = input else { continue };
        let syn::Pat::Ident(pat_id) = &*pt.pat else { continue };
        let arg_ident = &pat_id.ident;
        let arg_ty = &*pt.ty;

        let entry = registry.input_entry(arg_ty).unwrap_or_else(|| {
            panic!(
                "JniExt::on_function: input type `{}` for `{}` is unresolved",
                TypeKey::from_type(arg_ty),
                original_ident,
            )
        });
        let wire = &entry.destination;
        let conv = entry.function.sig.ident.clone();
        let wire_ident = if matches!(wire, syn::Type::Ptr(_)) {
            format_ident!("{}_ptr", arg_ident)
        } else {
            arg_ident.clone()
        };
        let wire_with_lifetime = annotate_jobject_with_lifetime(wire, "a");
        wire_params.push(quote!(#wire_ident: #wire_with_lifetime));
        // Input wrapper takes wires by ref except for raw pointers.
        if matches!(wire, syn::Type::Ptr(_)) {
            prelude.push(quote!(let #arg_ident = #conv(&mut env, #wire_ident)?;));
        } else {
            prelude.push(quote!(let #arg_ident = #conv(&mut env, &#wire_ident)?;));
        }
        if matches!(arg_ty, syn::Type::Reference(_)) {
            call_args.push(quote!(&#arg_ident));
        } else {
            call_args.push(quote!(#arg_ident));
        }
    }

    // Output: uniform path. Look up the registered converter for the
    // return type as-written (ReturnType::Default → `()`). The plugin's
    // own rank handlers cover `()`, `ZResult<T>`, `ZResult<()>`, etc.
    // No special branching here.
    let return_ty: syn::Type = match &f.sig.output {
        syn::ReturnType::Default => syn::parse_quote!(()),
        syn::ReturnType::Type(_, ty) => (**ty).clone(),
    };
    let output_entry = registry.output_entry(&return_ty).unwrap_or_else(|| {
        panic!(
            "JniExt::on_function: return type `{}` for `{}` is unresolved",
            TypeKey::from_type(&return_ty),
            original_ident,
        )
    });
    let wire_return_ty = output_entry.destination.clone();
    let conv = output_entry.function.sig.ident.clone();
    let wire_with_lifetime = annotate_jobject_with_lifetime(&wire_return_ty, "a");
    let wire_return = wire_with_lifetime.to_token_stream();
    let on_err: TokenStream = sentinel_for_wire(&wire_return_ty);

    let zresult = &ext.zresult;
    let call_expr = quote!(#source_module::#original_ident(#(#call_args),*));

    // Single body shape: bind `__result` to the source-fn return value
    // (whatever its type — `()`, `T`, `ZResult<T>`, etc.) and feed it
    // straight into the registered output converter, which handles any
    // unwrap / encode in one step. The closure return matches the
    // converter return.
    let body = quote! {
        {
            (|| -> #zresult<#wire_return> {
                #(#prelude)*
                let __result = #call_expr;
                #conv(&mut env, __result)
            })()
            .unwrap_or_else(|err| {
                #throw!(env, err);
                #on_err
            })
        }
    };

    quote! {
        #[no_mangle]
        #[allow(non_snake_case, unused_mut, unused_variables, dead_code)]
        pub unsafe extern "C" fn #wrapper_ident<'a>(
            mut env: jni::JNIEnv<'a>,
            _class: jni::objects::JClass<'a>,
            #(#wire_params),*
        ) -> #wire_return #body
    }
}

fn mangle_jni_name(ext: &JniExt, ident: &syn::Ident) -> syn::Ident {
    let camel = snake_to_camel(&ident.to_string());
    let mut name = ext.jni_class_path.clone();
    name.push('_');
    name.push_str(&camel);
    if !ext.jni_method_suffix.is_empty() {
        name.push_str(&ext.jni_method_suffix);
    }
    syn::Ident::new(&name, Span::call_site())
}

/// Sentinel value to return through the wrapper signature when the inner
/// closure errors. Must compile against any wire type we emit.
fn sentinel_for_wire(wire: &syn::Type) -> TokenStream {
    if let syn::Type::Path(tp) = wire {
        if let Some(last) = tp.path.segments.last() {
            let name = last.ident.to_string();
            return match name.as_str() {
                "jboolean" | "jbyte" | "jchar" | "jshort" | "jint" | "jlong" => quote!(0 as #wire),
                "jfloat" | "jdouble" => quote!(0.0 as #wire),
                "JObject" | "JString" | "JByteArray" | "JClass" => {
                    quote!(jni::objects::JObject::null().into())
                }
                _ => quote!(unsafe { std::mem::zeroed::<#wire>() }),
            };
        }
    }
    if matches!(wire, syn::Type::Ptr(_)) {
        return quote!(std::ptr::null());
    }
    quote!(unsafe { std::mem::zeroed::<#wire>() })
}

// ──────────────────────────────────────────────────────────────────────
// Primitive bodies
// ──────────────────────────────────────────────────────────────────────

fn primitive_input(ty: &syn::Type) -> Option<(syn::Type, syn::Expr)> {
    let key = TypeKey::from_type(ty).as_str().to_string();
    // Bodies receive `v: &<wire>`; primitives are Copy so `*v` works.
    Some(match key.as_str() {
        "bool" => (
            syn::parse_quote!(jni::sys::jboolean),
            syn::parse_quote!(*v != 0),
        ),
        "i64" => (
            syn::parse_quote!(jni::sys::jlong),
            syn::parse_quote!(*v),
        ),
        "f64" => (
            syn::parse_quote!(jni::sys::jdouble),
            syn::parse_quote!(*v),
        ),
        "Duration" | "std :: time :: Duration" => (
            syn::parse_quote!(jni::sys::jlong),
            syn::parse_quote!(std::time::Duration::from_millis(*v as u64)),
        ),
        "String" => (
            syn::parse_quote!(jni::objects::JString),
            syn::parse_quote!({
                let s = env
                    .get_string(v)
                    .map_err(|e| crate::errors::ZError(format!("decode_string: {}", e)))?;
                s.into()
            }),
        ),
        "Vec < u8 >" => (
            syn::parse_quote!(jni::objects::JByteArray),
            syn::parse_quote!({
                env.convert_byte_array(v)
                    .map_err(|e| crate::errors::ZError(format!("decode_byte_array: {}", e)))?
            }),
        ),
        _ => return None,
    })
}

fn primitive_output(ty: &syn::Type) -> Option<(syn::Type, syn::Expr)> {
    let key = TypeKey::from_type(ty).as_str().to_string();
    // Output wrappers take v by value (move). Primitives are Copy, so
    // `v as wire` works. String/Vec consume v.
    Some(match key.as_str() {
        "bool" => (
            syn::parse_quote!(jni::sys::jboolean),
            syn::parse_quote!(v as jni::sys::jboolean),
        ),
        "i64" => (
            syn::parse_quote!(jni::sys::jlong),
            syn::parse_quote!(v as jni::sys::jlong),
        ),
        "f64" => (
            syn::parse_quote!(jni::sys::jdouble),
            syn::parse_quote!(v as jni::sys::jdouble),
        ),
        "String" => (
            syn::parse_quote!(jni::objects::JString),
            syn::parse_quote!({
                env.new_string(v.as_str())
                    .map_err(|e| crate::errors::ZError(format!("encode_string: {}", e)))?
            }),
        ),
        "Vec < u8 >" => (
            syn::parse_quote!(jni::objects::JByteArray),
            syn::parse_quote!({
                env.byte_array_from_slice(v.as_slice())
                    .map_err(|e| crate::errors::ZError(format!("encode_byte_array: {}", e)))?
            }),
        ),
        _ => return None,
    })
}

// ──────────────────────────────────────────────────────────────────────
// Option<_> wrappers
// ──────────────────────────────────────────────────────────────────────

/// Build `Option<T>`'s input converter.
///
/// Two paths, picked in this order:
///
/// 1. **Niche path** (preferred). If `T`'s converter exposes any niche
///    slots, carve the first one and use it as the `None` discriminator.
///    The wrapper keeps `T`'s wire unchanged — no boxing, no extra
///    allocation, ABI-identical to a hand-written `if v == sentinel`.
///    The `rest` of the niche set is re-exported on the wrapper so an
///    enclosing wrapper (e.g. `Option<Option<T>>`) can keep carving.
///
/// 2. **Boxed-primitive fallback**. If `T`'s wire is a JNI primitive
///    (`jlong`, `jint`, …) and there is no niche, the wrapper widens
///    the wire to `JObject` carrying a Java boxed type (`java.lang.Long`,
///    `java.lang.Integer`, …). `null` denotes `None`. The wrapper
///    exposes no further niches — every `JObject` value already carries
///    meaning (null = None, non-null = Some).
///
/// If neither path applies (non-primitive wire, no niche), the wrap
/// fails and the resolver falls through to other rank-1 attempts.
fn option_input(
    t1: &syn::Type,
    registry: &Registry,
) -> Option<(syn::Type, syn::Expr, Niches)> {
    let inner_entry = registry.input_entry(t1)?;
    let inner_wire = inner_entry.destination.clone();
    let inner_conv = inner_entry.function.sig.ident.clone();

    // 1. Niche path.
    if let Some((slot, rest)) = inner_entry.niches.clone().carve() {
        let pred = &slot.matches;
        let body: syn::Expr = syn::parse_quote!({
            if #pred { None } else { Some(#inner_conv(env, v)?) }
        });
        return Some((inner_wire, body, rest));
    }

    // 2. Boxed-primitive fallback.
    if is_jni_primitive(&inner_wire) {
        let unbox_method = jni_unbox_method(&inner_wire);
        let unbox_sig = jni_unbox_sig(&inner_wire);
        let getter = jni_unbox_getter(&inner_wire);
        let getter_id = format_ident!("{}", getter);
        let body: syn::Expr = syn::parse_quote!({
            if !v.is_null() {
                let __unboxed: #inner_wire = env
                    .call_method(&v, #unbox_method, #unbox_sig, &[])
                    .and_then(|val| val.#getter_id())
                    .map_err(|e| crate::errors::ZError(format!("Option unbox: {}", e)))?;
                Some(#inner_conv(env, &__unboxed)?)
            } else {
                None
            }
        });
        let wire: syn::Type = syn::parse_quote!(jni::objects::JObject);
        return Some((wire, body, Niches::empty()));
    }

    None
}

/// Build `Option<T>`'s output converter — symmetric to [`option_input`].
fn option_output(
    t1: &syn::Type,
    registry: &Registry,
) -> Option<(syn::Type, syn::Expr, Niches)> {
    let inner_entry = registry.output_entry(t1)?;
    let inner_wire = inner_entry.destination.clone();
    let inner_conv = inner_entry.function.sig.ident.clone();

    // 1. Niche path.
    if let Some((slot, rest)) = inner_entry.niches.clone().carve() {
        let none_value = &slot.value;
        let body: syn::Expr = syn::parse_quote!({
            match v {
                Some(value) => #inner_conv(env, value)?,
                None => #none_value,
            }
        });
        return Some((inner_wire, body, rest));
    }

    // 2. Boxed-primitive fallback.
    if is_jni_primitive(&inner_wire) {
        let java_class = jni_box_class(&inner_wire);
        let box_sig = jni_box_sig(&inner_wire);
        let variant = jni_box_variant(&inner_wire);
        let variant_id = format_ident!("{}", variant);
        let body: syn::Expr = syn::parse_quote!({
            match v {
                Some(value) => {
                    let __raw: #inner_wire = #inner_conv(env, value)?;
                    env.call_static_method(
                        #java_class,
                        "valueOf",
                        #box_sig,
                        &[jni::objects::JValue::#variant_id(__raw)],
                    )
                    .and_then(|val| val.l())
                    .map_err(|e| crate::errors::ZError(format!("Option box: {}", e)))?
                }
                None => jni::objects::JObject::null(),
            }
        });
        let wire: syn::Type = syn::parse_quote!(jni::objects::JObject);
        return Some((wire, body, Niches::empty()));
    }

    None
}

// ──────────────────────────────────────────────────────────────────────
// Callback wrappers — impl Fn(args) -> JObject (Kotlin fun-interface)
// ──────────────────────────────────────────────────────────────────────

fn callback_input(
    ext: &JniExt,
    args: &[syn::Type],
    registry: &Registry,
) -> Option<(syn::Type, syn::Expr)> {
    let stem = derive_callback_stem(args);
    let kotlin_class = format!("JNI{}Callback", stem);
    let kotlin_fqn = if ext.kotlin_callback_package.is_empty() {
        kotlin_class.clone()
    } else {
        format!("{}.{}", ext.kotlin_callback_package, kotlin_class)
    };
    let internal_class = kotlin_fqn.replace('.', "/");

    // Per-arg: encode call + JNI signature chunk.
    let mut arg_idents: Vec<syn::Ident> = Vec::new();
    let mut arg_preludes: Vec<TokenStream> = Vec::new();
    let mut jvalue_exprs: Vec<TokenStream> = Vec::new();
    let mut sig = String::from("(");

    for (i, arg_ty) in args.iter().enumerate() {
        let raw_ident = format_ident!("__arg{}", i);
        let enc_ident = format_ident!("__arg{}_encoded", i);
        let obj_ident = format_ident!("__arg{}_obj", i);

        // Args are output-direction (encoded outbound). Look up output entry.
        let arg_entry = registry.output_entry(arg_ty)?;
        let arg_wire = arg_entry.destination.clone();
        let conv = arg_entry.function.sig.ident.clone();

        match jni_field_access(&arg_wire) {
            Some((s, _, false)) => {
                sig.push_str(s);
                arg_preludes.push(quote! {
                    let #raw_ident = &__cb_args.#i;
                    let #enc_ident = #conv(&mut env, #raw_ident)?;
                });
                jvalue_exprs.push(quote!(jni::objects::JValue::from(#enc_ident)));
            }
            Some((s, _, true)) => {
                sig.push_str(s);
                arg_preludes.push(quote! {
                    let #raw_ident = &__cb_args.#i;
                    let #enc_ident = #conv(&mut env, #raw_ident)?;
                    let #obj_ident: jni::objects::JObject = #enc_ident.into();
                });
                jvalue_exprs.push(quote!(jni::objects::JValue::Object(&#obj_ident)));
            }
            None if is_jobject_wire(&arg_wire) => {
                sig.push_str(&format!("L{};", internal_class));
                arg_preludes.push(quote! {
                    let #enc_ident = #conv(&mut env, &__cb_args.#i)?;
                    let #obj_ident: jni::objects::JObject = #enc_ident;
                });
                jvalue_exprs.push(quote!(jni::objects::JValue::Object(&#obj_ident)));
            }
            None => return None, // unsupported wire form
        }
        arg_idents.push(raw_ident);
    }
    sig.push_str(")V");

    // Tuple destructure for closure args.
    let arg_pat_ty: Vec<TokenStream> = args.iter().map(|t| quote!(#t)).collect();
    let arg_pat_ident: Vec<TokenStream> = (0..args.len())
        .map(|i| {
            let ident = format_ident!("__cb_arg{}", i);
            quote!(#ident)
        })
        .collect();
    let _ = arg_pat_ident;

    let zresult = &ext.zresult;
    let stem_lit = syn::LitStr::new(&stem, Span::call_site());
    let sig_lit = syn::LitStr::new(&sig, Span::call_site());

    // Body: capture global ref, return a Box<dyn Fn(args)>.
    // The wrapper takes the raw JObject `v` (the Kotlin callback ref).
    let arg_indices: Vec<syn::Index> = (0..args.len()).map(syn::Index::from).collect();
    let _ = arg_indices;

    // Build the Fn closure body.
    let arg_names: Vec<syn::Ident> = (0..args.len())
        .map(|i| format_ident!("__cb_arg{}", i))
        .collect();

    // Convert (self.0, .1, ...) tuple field accesses into __cb_arg0, _arg1.
    // Replace `__cb_args.0` with `__cb_arg0` etc. in arg_preludes by
    // re-rendering: easier to just rebuild here.
    let mut fixed_preludes: Vec<TokenStream> = Vec::new();
    for (i, arg_ty) in args.iter().enumerate() {
        let raw_ident = format_ident!("__arg{}", i);
        let enc_ident = format_ident!("__arg{}_encoded", i);
        let obj_ident = format_ident!("__arg{}_obj", i);
        let cb_arg = &arg_names[i];
        let arg_entry = registry.output_entry(arg_ty)?;
        let arg_wire = arg_entry.destination.clone();
        let conv = arg_entry.function.sig.ident.clone();
        // Output wrappers take rust by value (move). cb_arg is the
        // closure parameter (by value), so pass it directly.
        match jni_field_access(&arg_wire) {
            Some((_, _, false)) => fixed_preludes.push(quote! {
                let #enc_ident = #conv(&mut env, #cb_arg)?;
            }),
            Some((_, _, true)) => fixed_preludes.push(quote! {
                let #enc_ident = #conv(&mut env, #cb_arg)?;
                let #obj_ident: jni::objects::JObject = #enc_ident.into();
            }),
            None if is_jobject_wire(&arg_wire) => fixed_preludes.push(quote! {
                let #enc_ident = #conv(&mut env, #cb_arg)?;
                let #obj_ident: jni::objects::JObject = #enc_ident;
            }),
            None => return None,
        }
        let _ = raw_ident; // unused with by-value flow
    }

    let body: syn::Expr = syn::parse_quote!({
        use std::sync::Arc;
        let java_vm = Arc::new(env.get_java_vm()
            .map_err(|e| crate::errors::ZError(format!("Unable to retrieve JVM: {}", e)))?);
        let callback_global_ref = env.new_global_ref(&v)
            .map_err(|e| crate::errors::ZError(format!("Unable to global-ref callback: {}", e)))?;
        Box::new(move |#(#arg_names: #arg_pat_ty),*| {
            let _ = (|| -> #zresult<()> {
                let mut env = java_vm
                    .attach_current_thread_as_daemon()
                    .map_err(|e| crate::errors::ZError(format!("Attach thread for {}: {}", #stem_lit, e)))?;
                #(#fixed_preludes)*
                env.call_method(
                    &callback_global_ref,
                    "run",
                    #sig_lit,
                    &[#(#jvalue_exprs),*],
                )
                .map_err(|e| {
                    let _ = env.exception_describe();
                    crate::errors::ZError(e.to_string())
                })?;
                Ok(())
            })()
            .map_err(|e| tracing::error!("On {} callback error: {e}", #stem_lit));
        })
    });

    // The destination type for an `impl Fn(args)` parameter is JObject (the
    // Kotlin callback object). We return Box<dyn Fn(args) + Send + Sync>
    // wrapped in a generic so it satisfies the impl-trait param type.
    // Actually the SOURCE (rust) type IS `impl Fn(args) + Send + Sync + 'static`,
    // so the wrapper's return type is that. Box<dyn Fn> coerces.
    Some((syn::parse_quote!(jni::objects::JObject), body))
}

fn derive_callback_stem(args: &[syn::Type]) -> String {
    if args.is_empty() {
        return "Empty".into();
    }
    let mut s = String::new();
    for a in args {
        s.push_str(&type_short_ident(a));
    }
    s
}

fn type_short_ident(ty: &syn::Type) -> String {
    if let syn::Type::Path(tp) = ty {
        if let Some(last) = tp.path.segments.last() {
            return last.ident.to_string();
        }
    }
    "Unknown".into()
}

fn is_jobject_wire(wire: &syn::Type) -> bool {
    if let syn::Type::Path(tp) = wire {
        if let Some(last) = tp.path.segments.last() {
            return last.ident == "JObject";
        }
    }
    false
}

/// True if `wire` is a JNI handle (`JObject`, `JString`, `JByteArray`,
/// `JClass`) that natively supports a `null` discriminator. These types
/// all impl `is_null()` and accept `JObject::null().into()` for
/// construction.
fn is_jobject_shaped_wire(wire: &syn::Type) -> bool {
    if let syn::Type::Path(tp) = wire {
        if let Some(last) = tp.path.segments.last() {
            return matches!(
                last.ident.to_string().as_str(),
                "JObject" | "JString" | "JByteArray" | "JClass"
            );
        }
    }
    false
}

/// Default niche set for a JNI wrapper wire: every `J*` handle has a
/// genuine `null` value that no live conversion ever produces, so wrap
/// it as a single niche; everything else (`jlong`, `jint`, `()`, …) has
/// no implicit niche.
///
/// Plugins are free to declare *additional* niches on top of this for
/// pointer-shape primitives like `Arc::into_raw`-as-`jlong`.
fn default_niches_for_wire(wire: &syn::Type) -> Niches {
    if is_jobject_shaped_wire(wire) {
        Niches::one(
            syn::parse_quote!(jni::objects::JObject::null().into()),
            syn::parse_quote!(v.is_null()),
        )
    } else {
        Niches::empty()
    }
}

// ──────────────────────────────────────────────────────────────────────
// Struct rank-0 bodies
// ──────────────────────────────────────────────────────────────────────

fn struct_input_body(
    ext: &JniExt,
    s: &syn::ItemStruct,
    registry: &Registry,
) -> Option<(syn::Type, syn::Expr)> {
    let struct_name = s.ident.to_string();
    let struct_module = struct_module_path(ext, s);
    let struct_ident = &s.ident;

    let syn::Fields::Named(named) = &s.fields else {
        return None;
    };

    let mut field_preludes: Vec<TokenStream> = Vec::new();
    let mut field_init: Vec<TokenStream> = Vec::new();

    for field in &named.named {
        let fname_ident = field.ident.as_ref().unwrap().clone();
        let fname = fname_ident.to_string();
        let camel = snake_to_camel(&fname);
        let err_prefix = format!("{struct_name}.{camel}: {{}}");
        let raw_ident = format_ident!("__{}_raw", fname_ident);

        // Defer if any field's input converter isn't resolved yet — the
        // fixed-point loop will retry on the next iteration.
        let field_entry = registry.input_entry(&field.ty)?;
        let field_wire = field_entry.destination.clone();
        let field_conv = field_entry.function.sig.ident.clone();

        match jni_field_access(&field_wire) {
            Some((sig, accessor, false)) => {
                field_preludes.push(quote! {
                    let #raw_ident: #field_wire = env.get_field(v, #camel, #sig)
                        .and_then(|val| val.#accessor())
                        .map_err(|e| crate::errors::ZError(format!(#err_prefix, e)))? as _;
                    let #fname_ident = #field_conv(env, &#raw_ident)?;
                });
            }
            Some((sig, _, true)) => {
                let tmp_ident = format_ident!("__{}_jobj", fname_ident);
                field_preludes.push(quote! {
                    let #tmp_ident: jni::objects::JObject = env.get_field(v, #camel, #sig)
                        .and_then(|val| val.l())
                        .map_err(|e| crate::errors::ZError(format!(#err_prefix, e)))?;
                    let #raw_ident: #field_wire = #tmp_ident.into();
                    let #fname_ident = #field_conv(env, &#raw_ident)?;
                });
            }
            None => {
                // Wire is JObject — fetch via .l() and pass by reference.
                field_preludes.push(quote! {
                    let #raw_ident: jni::objects::JObject = env.get_field(v, #camel, "Ljava/lang/Object;")
                        .and_then(|val| val.l())
                        .map_err(|e| crate::errors::ZError(format!(#err_prefix, e)))?;
                    let #fname_ident = #field_conv(env, &#raw_ident)?;
                });
            }
        }
        field_init.push(quote!(#fname_ident));
    }

    let body: syn::Expr = syn::parse_quote!({
        #(#field_preludes)*
        #struct_module::#struct_ident { #(#field_init),* }
    });
    Some((syn::parse_quote!(jni::objects::JObject), body))
}

fn struct_output_body(
    ext: &JniExt,
    s: &syn::ItemStruct,
    registry: &Registry,
) -> Option<(syn::Type, syn::Expr)> {
    let struct_name = s.ident.to_string();
    let java_class_name = if ext.java_class_prefix.is_empty() {
        struct_name.clone()
    } else {
        format!("{}/{}", ext.java_class_prefix, struct_name)
    };

    let syn::Fields::Named(named) = &s.fields else {
        return None;
    };

    let mut field_preludes: Vec<TokenStream> = Vec::new();
    let mut ctor_args: Vec<TokenStream> = Vec::new();
    let mut ctor_sig = String::from("(");

    for field in &named.named {
        let fname_ident = field.ident.as_ref().unwrap().clone();
        let field_value_ident = format_ident!("__{}_value", fname_ident);
        let encoded_ident = format_ident!("__{}_encoded", fname_ident);
        let encoded_obj_ident = format_ident!("__{}_encoded_obj", fname_ident);

        // Defer if any field's output converter isn't resolved yet.
        let field_entry = registry.output_entry(&field.ty)?;
        let field_wire = field_entry.destination.clone();
        let field_conv = field_entry.function.sig.ident.clone();

        field_preludes.push(quote! {
            let #field_value_ident = v.#fname_ident.clone();
            let #encoded_ident = #field_conv(env, #field_value_ident)?;
        });

        match jni_field_access(&field_wire) {
            Some((sig, _, false)) => {
                ctor_sig.push_str(sig);
                ctor_args.push(quote!(jni::objects::JValue::from(#encoded_ident)));
            }
            Some((sig, _, true)) => {
                ctor_sig.push_str(sig);
                field_preludes.push(quote! {
                    let #encoded_obj_ident: jni::objects::JObject = #encoded_ident.into();
                });
                ctor_args.push(quote!(jni::objects::JValue::Object(&#encoded_obj_ident)));
            }
            None => {
                ctor_sig.push_str("Ljava/lang/Object;");
                field_preludes.push(quote! {
                    let #encoded_obj_ident: jni::objects::JObject = #encoded_ident;
                });
                ctor_args.push(quote!(jni::objects::JValue::Object(&#encoded_obj_ident)));
            }
        }
    }
    ctor_sig.push_str(")V");
    let ctor_sig_lit = syn::LitStr::new(&ctor_sig, Span::call_site());

    let body: syn::Expr = syn::parse_quote!({
        #(#field_preludes)*
        let __obj = env.new_object(
            #java_class_name,
            #ctor_sig_lit,
            &[#(#ctor_args),*],
        )
        .map_err(|e| crate::errors::ZError(format!("encode struct: {}", e)))?;
        __obj
    });
    Some((syn::parse_quote!(jni::objects::JObject), body))
}

fn struct_module_path(ext: &JniExt, s: &syn::ItemStruct) -> syn::Path {
    // Place the struct under <source_module>::<file_stem>::<Name>. Today's
    // pipeline derives the module from the source file stem; here we ride
    // on the same convention by inspecting the SourceLocation. Without a
    // location handy at this stage we fall back to <source_module>::<Name>.
    // In practice the actual file stem is added in the compose step at the
    // call site by the consuming crate when needed.
    let _ = s;
    ext.source_module.clone()
}

// ──────────────────────────────────────────────────────────────────────
// JNI primitive (un)boxing helpers
// ──────────────────────────────────────────────────────────────────────

fn is_jni_primitive(ty: &syn::Type) -> bool {
    if let syn::Type::Path(tp) = ty {
        if let Some(last) = tp.path.segments.last() {
            let name = last.ident.to_string();
            return matches!(
                name.as_str(),
                "jboolean" | "jbyte" | "jchar" | "jshort" | "jint" | "jlong" | "jfloat" | "jdouble"
            );
        }
    }
    false
}

fn jni_box_class(wire: &syn::Type) -> &'static str {
    match jni_prim_name(wire) {
        "jboolean" => "java/lang/Boolean",
        "jbyte" => "java/lang/Byte",
        "jchar" => "java/lang/Character",
        "jshort" => "java/lang/Short",
        "jint" => "java/lang/Integer",
        "jlong" => "java/lang/Long",
        "jfloat" => "java/lang/Float",
        "jdouble" => "java/lang/Double",
        _ => panic!("not a JNI primitive: {}", wire.to_token_stream()),
    }
}

fn jni_box_sig(wire: &syn::Type) -> &'static str {
    match jni_prim_name(wire) {
        "jboolean" => "(Z)Ljava/lang/Boolean;",
        "jbyte" => "(B)Ljava/lang/Byte;",
        "jchar" => "(C)Ljava/lang/Character;",
        "jshort" => "(S)Ljava/lang/Short;",
        "jint" => "(I)Ljava/lang/Integer;",
        "jlong" => "(J)Ljava/lang/Long;",
        "jfloat" => "(F)Ljava/lang/Float;",
        "jdouble" => "(D)Ljava/lang/Double;",
        _ => unreachable!(),
    }
}

fn jni_box_variant(wire: &syn::Type) -> &'static str {
    match jni_prim_name(wire) {
        "jboolean" => "Bool",
        "jbyte" => "Byte",
        "jchar" => "Char",
        "jshort" => "Short",
        "jint" => "Int",
        "jlong" => "Long",
        "jfloat" => "Float",
        "jdouble" => "Double",
        _ => unreachable!(),
    }
}

fn jni_unbox_method(wire: &syn::Type) -> &'static str {
    match jni_prim_name(wire) {
        "jboolean" => "booleanValue",
        "jbyte" => "byteValue",
        "jchar" => "charValue",
        "jshort" => "shortValue",
        "jint" => "intValue",
        "jlong" => "longValue",
        "jfloat" => "floatValue",
        "jdouble" => "doubleValue",
        _ => unreachable!(),
    }
}

fn jni_unbox_sig(wire: &syn::Type) -> &'static str {
    match jni_prim_name(wire) {
        "jboolean" => "()Z",
        "jbyte" => "()B",
        "jchar" => "()C",
        "jshort" => "()S",
        "jint" => "()I",
        "jlong" => "()J",
        "jfloat" => "()F",
        "jdouble" => "()D",
        _ => unreachable!(),
    }
}

fn jni_unbox_getter(wire: &syn::Type) -> &'static str {
    match jni_prim_name(wire) {
        "jboolean" => "z",
        "jbyte" => "b",
        "jchar" => "c",
        "jshort" => "s",
        "jint" => "i",
        "jlong" => "j",
        "jfloat" => "f",
        "jdouble" => "d",
        _ => unreachable!(),
    }
}

fn jni_prim_name(wire: &syn::Type) -> &str {
    if let syn::Type::Path(tp) = wire {
        if let Some(last) = tp.path.segments.last() {
            return Box::leak(last.ident.to_string().into_boxed_str());
        }
    }
    "<not a path>"
}

/// If `ty` is a `&T` borrow with no explicit lifetime, splice in `'<life>`.
/// Otherwise return `ty` unchanged.
fn annotate_borrow_with_lifetime(ty: &syn::Type, life: &str) -> syn::Type {
    if let syn::Type::Reference(r) = ty {
        if r.lifetime.is_none() {
            let mut new = r.clone();
            new.lifetime = Some(syn::Lifetime::new(&format!("'{}", life), proc_macro2::Span::call_site()));
            return syn::Type::Reference(new);
        }
    }
    ty.clone()
}

/// If `ty` is `JObject` / `JString` / `JByteArray` (no explicit angle args),
/// splice in `<'<life>>`. Otherwise return `ty` unchanged.
fn annotate_jobject_with_lifetime(ty: &syn::Type, life: &str) -> syn::Type {
    if let syn::Type::Path(tp) = ty {
        if let Some(last) = tp.path.segments.last() {
            let name = last.ident.to_string();
            if matches!(name.as_str(), "JObject" | "JString" | "JByteArray" | "JClass") {
                if matches!(last.arguments, syn::PathArguments::None) {
                    let mut new = tp.clone();
                    if let Some(last) = new.path.segments.last_mut() {
                        let lt = syn::Lifetime::new(&format!("'{}", life), proc_macro2::Span::call_site());
                        last.arguments = syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                            colon2_token: None,
                            lt_token: syn::token::Lt::default(),
                            args: syn::punctuated::Punctuated::from_iter(std::iter::once(syn::GenericArgument::Lifetime(lt))),
                            gt_token: syn::token::Gt::default(),
                        });
                    }
                    return syn::Type::Path(new);
                }
            }
        }
    }
    ty.clone()
}

// ──────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────

fn pat_match(ty: &syn::Type, pat: &str) -> bool {
    ty.to_token_stream().to_string() == pat
}

fn bare_path_ident(ty: &syn::Type) -> Option<syn::Ident> {
    if let syn::Type::Path(tp) = ty {
        if let Some(last) = tp.path.segments.last() {
            if matches!(last.arguments, syn::PathArguments::None) {
                return Some(last.ident.clone());
            }
        }
    }
    None
}


// ──────────────────────────────────────────────────────────────────────
// JNI-internal naming convention. Hand-written code in zenoh-jni
// (e.g. liveliness.rs, advanced_subscriber.rs) calls auto-generated
// converters by these computed names — so the convention is part of the
// JNI plugin's public contract, not a private implementation detail.
// ──────────────────────────────────────────────────────────────────────

/// INPUT: wire → rust. Format `<wire_id>_to_<rust_id>_<hash>`. Special
/// case: `impl Fn(...)` keeps the legacy `process_kotlin_<Stem>_callback`
/// name so existing hand-written call sites continue to resolve.
fn input_name(rust: &syn::Type, wire: &syn::Type) -> syn::Ident {
    if let Some(args) = extract_fn_trait_args(rust) {
        let stem = derive_callback_stem(&args);
        let s = format!("process_kotlin_{}_callback", stem);
        return syn::Ident::new(&s, Span::call_site());
    }
    let rust_id = sanitize_for_ident(&rust.to_token_stream().to_string());
    let wire_id = wire_short(wire);
    let h = hash_pair(rust, wire);
    let s = format!("{}_to_{}_{:08x}", wire_id, rust_id, h & 0xffff_ffff);
    syn::Ident::new(&s, Span::call_site())
}

/// OUTPUT: rust → wire. Format `<rust_id>_to_<wire_id>_<hash>`.
fn output_name(rust: &syn::Type, wire: &syn::Type) -> syn::Ident {
    let rust_id = sanitize_for_ident(&rust.to_token_stream().to_string());
    let wire_id = wire_short(wire);
    let h = hash_pair(rust, wire);
    let s = format!("{}_to_{}_{:08x}", rust_id, wire_id, h & 0xffff_ffff);
    syn::Ident::new(&s, Span::call_site())
}

fn sanitize_for_ident(s: &str) -> String {
    // Special-case the empty tuple — the all-punctuation token stream
    // would sanitize to a meaningless fallback. `unit` is recognisable.
    if s.trim() == "()" {
        return "unit".to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut prev_underscore = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            prev_underscore = false;
        } else if !prev_underscore {
            out.push('_');
            prev_underscore = true;
        }
    }
    while out.starts_with('_') {
        out.remove(0);
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("ty");
    }
    if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

fn wire_short(wire: &syn::Type) -> String {
    if let syn::Type::Path(tp) = wire {
        if let Some(last) = tp.path.segments.last() {
            return sanitize_for_ident(&last.ident.to_string());
        }
    }
    sanitize_for_ident(&wire.to_token_stream().to_string())
}

fn hash_pair(rust: &syn::Type, wire: &syn::Type) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    rust.to_token_stream().to_string().hash(&mut h);
    "::".hash(&mut h);
    wire.to_token_stream().to_string().hash(&mut h);
    h.finish()
}

/// Reconstruct the `impl Fn(args...) + Send + Sync + 'static` syn::Type
/// from a flat slice of arg types. Used by the rank-1/2/3 callback impls
/// to feed `input_wrapper` the original outer type.
fn build_fn_type(args: &[syn::Type]) -> syn::Type {
    let arg_iter = args.iter();
    syn::parse_quote!(impl Fn( #(#arg_iter),* ) + Send + Sync + 'static)
}

// ──────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────
//
// These tests exercise the niche cascade by hand-building registry
// entries with deliberate niche shapes, then driving `option_input` /
// `option_output` directly. They mirror the documented `Niches`
// semantics: each `Option<_>` layer carves one slot and re-exports the
// rest; once the rest is exhausted, the next layer falls back to the
// boxed-Java-primitive scheme.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::niches::{NicheSlot, Niches};
    use crate::core::registry::{Registry, TypeEntry, TypeKey};
    use quote::ToTokens;

    /// Build a `TypeEntry` for use in tests. The function body is not
    /// inspected by `option_input` / `option_output`; only the ident,
    /// destination, and niches matter, so we use a stub `ItemFn`.
    fn entry(wire: syn::Type, conv_name: &str, niches: Niches) -> TypeEntry {
        let ident = syn::Ident::new(conv_name, proc_macro2::Span::call_site());
        let func: syn::ItemFn = syn::parse_quote!(
            unsafe fn #ident<'env, 'v>(
                env: &mut jni::JNIEnv<'env>,
                v: &#wire,
            ) -> crate::errors::ZResult<()> {
                Ok(())
            }
        );
        TypeEntry {
            destination: wire,
            function: func,
            subs: vec![],
            required: false,
            niches,
        }
    }

    fn install_input(reg: &mut Registry, ty_str: &str, rank: usize, e: TypeEntry) {
        reg.input_types[rank].insert(TypeKey::parse(ty_str), Some(e));
    }
    fn install_output(reg: &mut Registry, ty_str: &str, rank: usize, e: TypeEntry) {
        reg.output_types[rank].insert(TypeKey::parse(ty_str), Some(e));
    }

    /// Single niche, single Option layer — wire stays the inner wire,
    /// remainder is empty. No widening to JObject.
    #[test]
    fn option_carves_single_niche() {
        let mut reg = Registry::default();
        install_input(
            &mut reg,
            "TestType",
            0,
            entry(
                syn::parse_quote!(jni::sys::jlong),
                "jlong_to_TestType_aaaa",
                Niches::one(syn::parse_quote!(0i64), syn::parse_quote!(*v == 0)),
            ),
        );

        let inner_ty: syn::Type = syn::parse_quote!(TestType);
        let (wire, _body, niches) = option_input(&inner_ty, &reg).expect("Option<TestType> resolves");

        assert_eq!(
            wire.to_token_stream().to_string(),
            "jni :: sys :: jlong",
            "wire stays jlong (no JObject widening)"
        );
        assert!(niches.is_empty(), "single niche fully consumed");
    }

    /// Two niches, two cascading Option layers, both stay on the same
    /// wire. The third layer hits empty niches and falls back to box.
    #[test]
    fn option_cascades_through_multi_niche() {
        let mut reg = Registry::default();

        // TestType: jint with two niches (MIN, MAX).
        install_input(
            &mut reg,
            "TestType",
            0,
            entry(
                syn::parse_quote!(jni::sys::jint),
                "jint_to_TestType_aaaa",
                Niches::from_slots([
                    NicheSlot {
                        value: syn::parse_quote!(jni::sys::jint::MIN),
                        matches: syn::parse_quote!(*v == jni::sys::jint::MIN),
                    },
                    NicheSlot {
                        value: syn::parse_quote!(jni::sys::jint::MAX),
                        matches: syn::parse_quote!(*v == jni::sys::jint::MAX),
                    },
                ]),
            ),
        );

        // Layer 1: Option<TestType>.
        let layer1_ty: syn::Type = syn::parse_quote!(TestType);
        let (w1, _, n1) = option_input(&layer1_ty, &reg).expect("layer 1 resolves");
        assert_eq!(w1.to_token_stream().to_string(), "jni :: sys :: jint");
        assert_eq!(n1.len(), 1, "first carve leaves one niche");

        // Install the layer-1 wrapper as a rank-1 entry so layer-2 can
        // look it up. (In the real resolver this happens automatically;
        // here we mimic it by installing the produced ConverterImpl.)
        install_input(
            &mut reg,
            "Option < TestType >",
            1,
            entry(w1.clone(), "jint_to_OptionTestType_bbbb", n1),
        );

        // Layer 2: Option<Option<TestType>>.
        let layer2_ty: syn::Type = syn::parse_quote!(Option<TestType>);
        let (w2, _, n2) = option_input(&layer2_ty, &reg).expect("layer 2 resolves");
        assert_eq!(
            w2.to_token_stream().to_string(),
            "jni :: sys :: jint",
            "wire still jint at layer 2 — no widening"
        );
        assert!(n2.is_empty(), "second carve consumes the last niche");

        // Install layer-2 wrapper for the layer-3 lookup.
        install_input(
            &mut reg,
            "Option < Option < TestType > >",
            1,
            entry(w2.clone(), "jint_to_OptionOptionTestType_cccc", n2),
        );

        // Layer 3: Option<Option<Option<TestType>>>. No niches left,
        // inner wire is jint (a JNI primitive) → boxed-Long fallback.
        let layer3_ty: syn::Type = syn::parse_quote!(Option<Option<TestType>>);
        let (w3, _, n3) = option_input(&layer3_ty, &reg).expect("layer 3 resolves via box fallback");
        assert_eq!(
            w3.to_token_stream().to_string(),
            "jni :: objects :: JObject",
            "layer 3 widens to JObject (box fallback)"
        );
        assert!(
            n3.is_empty(),
            "boxed wrapper exposes no further niches — every JObject carries meaning"
        );
    }

    /// Output side mirrors input: niche values are emitted in the
    /// `None` arm of the match, and the remainder is re-exported.
    #[test]
    fn option_output_cascades_through_multi_niche() {
        let mut reg = Registry::default();
        install_output(
            &mut reg,
            "TestType",
            0,
            entry(
                syn::parse_quote!(jni::sys::jint),
                "TestType_to_jint_aaaa",
                Niches::from_slots([
                    NicheSlot {
                        value: syn::parse_quote!(-1i32),
                        matches: syn::parse_quote!(*v == -1),
                    },
                    NicheSlot {
                        value: syn::parse_quote!(-2i32),
                        matches: syn::parse_quote!(*v == -2),
                    },
                ]),
            ),
        );

        let inner_ty: syn::Type = syn::parse_quote!(TestType);
        let (w1, body1, n1) =
            option_output(&inner_ty, &reg).expect("Option<TestType> output resolves");
        assert_eq!(w1.to_token_stream().to_string(), "jni :: sys :: jint");
        assert_eq!(n1.len(), 1, "one slot left after carving the first");
        // The body must reference the carved value (-1) in the None arm.
        let body_str = body1.to_token_stream().to_string();
        assert!(
            body_str.contains("None => - 1i32") || body_str.contains("None => -1i32"),
            "expected `None => -1i32` in body; got:\n{}",
            body_str,
        );

        install_output(
            &mut reg,
            "Option < TestType >",
            1,
            entry(w1.clone(), "OptionTestType_to_jint_bbbb", n1),
        );

        let layer2_ty: syn::Type = syn::parse_quote!(Option<TestType>);
        let (w2, body2, n2) =
            option_output(&layer2_ty, &reg).expect("Option<Option<TestType>> output resolves");
        assert_eq!(w2.to_token_stream().to_string(), "jni :: sys :: jint");
        assert!(n2.is_empty());
        let body2_str = body2.to_token_stream().to_string();
        assert!(
            body2_str.contains("None => - 2i32") || body2_str.contains("None => -2i32"),
            "second layer must use the second niche (-2); got:\n{}",
            body2_str,
        );
    }

    /// JObject-shaped wires get the implicit `null` niche via
    /// [`default_niches_for_wire`], so `Option<T>` over a struct
    /// decoder stays on `JObject` (no boxing).
    #[test]
    fn option_over_jobject_uses_default_null_niche() {
        let mut reg = Registry::default();
        install_input(
            &mut reg,
            "MyStruct",
            0,
            entry(
                syn::parse_quote!(jni::objects::JObject),
                "JObject_to_MyStruct_aaaa",
                default_niches_for_wire(&syn::parse_quote!(jni::objects::JObject)),
            ),
        );

        let ty: syn::Type = syn::parse_quote!(MyStruct);
        let (wire, _, rest) = option_input(&ty, &reg).expect("Option<MyStruct> resolves");
        assert_eq!(wire.to_token_stream().to_string(), "jni :: objects :: JObject");
        assert!(rest.is_empty(), "JObject's single null niche is consumed");
    }

    /// No niche AND non-primitive wire → wrap fails (resolver falls
    /// through). Demonstrates that the boxed fallback only kicks in for
    /// JNI primitives.
    #[test]
    fn option_fails_when_no_niche_and_non_primitive_wire() {
        let mut reg = Registry::default();
        install_input(
            &mut reg,
            "MyStruct",
            0,
            entry(
                syn::parse_quote!(jni::objects::JObject),
                "JObject_to_MyStruct_aaaa",
                Niches::empty(), // explicit empty — author opted out
            ),
        );
        let ty: syn::Type = syn::parse_quote!(MyStruct);
        assert!(option_input(&ty, &reg).is_none());
    }

    /// Boxed fallback widens to `JObject` and exposes no further
    /// niches — protects callers from cascading when a layer has had
    /// to widen.
    #[test]
    fn option_box_fallback_exposes_no_niches() {
        let mut reg = Registry::default();
        install_input(
            &mut reg,
            "i64",
            0,
            entry(
                syn::parse_quote!(jni::sys::jlong),
                "jlong_to_i64_aaaa",
                Niches::empty(), // primitive `i64` — no niche
            ),
        );
        let ty: syn::Type = syn::parse_quote!(i64);
        let (wire, _, rest) = option_input(&ty, &reg).expect("Option<i64> via box fallback");
        assert_eq!(wire.to_token_stream().to_string(), "jni :: objects :: JObject");
        assert!(rest.is_empty());
    }
}
