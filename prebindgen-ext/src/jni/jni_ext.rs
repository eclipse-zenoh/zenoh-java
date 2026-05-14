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

use crate::core::converter_name::{input_name, output_name};
use crate::core::prebindgen_ext::PrebindgenExt;
use crate::core::registry::{extract_fn_trait_args, result_inner, Registry, TypeKey};
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
// PrebindgenExt impl
// ──────────────────────────────────────────────────────────────────────

impl PrebindgenExt for JniExt {
    // ── Wrapper assembly ─────────────────────────────────────────────

    fn wrap_input_converter(
        &self,
        name: &syn::Ident,
        rust: &syn::Type,
        wire: &syn::Type,
        body: &syn::Expr,
    ) -> syn::ItemFn {
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

    fn wrap_output_converter(
        &self,
        name: &syn::Ident,
        rust: &syn::Type,
        wire: &syn::Type,
        body: &syn::Expr,
    ) -> syn::ItemFn {
        let zresult = &self.zresult;
        let wire_with_lifetime = annotate_jobject_with_lifetime(wire, "a");
        // Output wrappers take rust by value (move) — handles like
        // Subscriber<()> don't implement Clone, so body can't go through
        // (*v).clone(). Bodies that need to consume v can move it.
        syn::parse_quote!(
            #[allow(non_snake_case, unused_mut, unused_variables, unused_braces, dead_code)]
            pub(crate) unsafe fn #name<'a>(env: &mut jni::JNIEnv<'a>, v: #rust) -> #zresult<#wire_with_lifetime> {
                Ok(#body)
            }
        )
    }

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
    ) -> Option<(syn::Type, syn::Expr)> {
        // Primitives.
        if let Some(b) = primitive_input(ty) {
            return Some(b);
        }
        // Bare-ident type that names a #[prebindgen] struct → build decode.
        if let Some(name) = bare_path_ident(ty) {
            if let Some((s, _)) = registry.structs.get(&name) {
                return struct_input_body(self, s, registry);
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
    ) -> Option<(syn::Type, syn::Expr)> {
        // Option<_>
        if pat_match(pat, "Option < _ >") {
            return option_input(t1, registry);
        }
        // impl Fn(_) + Send + Sync + 'static
        if let Some(args) = extract_fn_trait_args(pat) {
            if args.len() == 1 {
                return callback_input(self, std::slice::from_ref(t1), registry);
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
    ) -> Option<(syn::Type, syn::Expr)> {
        if let Some(args) = extract_fn_trait_args(pat) {
            if args.len() == 2 {
                return callback_input(self, &[t1.clone(), t2.clone()], registry);
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
    ) -> Option<(syn::Type, syn::Expr)> {
        if let Some(args) = extract_fn_trait_args(pat) {
            if args.len() == 3 {
                return callback_input(self, &[t1.clone(), t2.clone(), t3.clone()], registry);
            }
        }
        None
    }

    // ── Output converters ────────────────────────────────────────────

    fn on_output_type_rank_0(
        &self,
        ty: &syn::Type,
        registry: &Registry,
    ) -> Option<(syn::Type, syn::Expr)> {
        if let Some(b) = primitive_output(ty) {
            return Some(b);
        }
        if let Some(name) = bare_path_ident(ty) {
            if let Some((s, _)) = registry.structs.get(&name) {
                return struct_output_body(self, s, registry);
            }
        }
        None
    }

    fn on_output_type_rank_1(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        registry: &Registry,
    ) -> Option<(syn::Type, syn::Expr)> {
        // ZResult<T> never appears as an output entry — scan unwraps it.
        // Option<_> output
        if pat_match(pat, "Option < _ >") {
            return option_output(t1, registry);
        }
        None
    }

    fn on_output_type_rank_2(
        &self,
        _pat: &syn::Type,
        _t1: &syn::Type,
        _t2: &syn::Type,
        _registry: &Registry,
    ) -> Option<(syn::Type, syn::Expr)> {
        None
    }

    fn on_output_type_rank_3(
        &self,
        _pat: &syn::Type,
        _t1: &syn::Type,
        _t2: &syn::Type,
        _t3: &syn::Type,
        _registry: &Registry,
    ) -> Option<(syn::Type, syn::Expr)> {
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

    for input in &f.sig.inputs {
        let syn::FnArg::Typed(pt) = input else { continue };
        let syn::Pat::Ident(pat_id) = &*pt.pat else { continue };
        let arg_ident = &pat_id.ident;
        let arg_ty = &*pt.ty;

        // For `&T` borrow params: look up T's converter (not &T's), then
        // synthesize the borrow at the call site. Avoids the impossible
        // task of having a converter return a `&T` (it has nowhere to
        // borrow from). ZenohJniExt's `&Config`/`&Session` etc. arms
        // become unnecessary — KeyExpr/Config/Session's own input arms
        // (or auto-generation) cover them.
        let lookup_ty: syn::Type = match arg_ty {
            syn::Type::Reference(r) => (*r.elem).clone(),
            _ => arg_ty.clone(),
        };
        let key = TypeKey::from_type(&lookup_ty);
        let entry = lookup_input_resolved_by_key(registry, &key).unwrap_or_else(|| {
            panic!(
                "JniExt::on_function: input type `{}` (lookup for `{}`) for `{}` is unresolved",
                key,
                arg_ty.to_token_stream(),
                original_ident,
            )
        });
        let wire = &entry.destination;
        let conv = input_name(&lookup_ty, wire);
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

    let (wire_return, wrap_ok, on_err): (TokenStream, TokenStream, TokenStream) =
        match &f.sig.output {
            syn::ReturnType::Default => (quote!(()), quote!(Ok(())), quote!(())),
            syn::ReturnType::Type(_, ty) if is_unit(ty) || is_result_of_unit(ty) => {
                (quote!(()), quote!(Ok(())), quote!(()))
            }
            syn::ReturnType::Type(_, ty) => {
                // The body strategy unwraps ZResult<T> via `?`, so the
                // encoder receives the unwrapped T. Look up T's converter,
                // not ZResult<T>'s.
                let lookup_ty = result_inner(ty).unwrap_or_else(|| (**ty).clone());
                let key = TypeKey::from_type(&lookup_ty);
                let entry = lookup_output_resolved_by_key(registry, &key).unwrap_or_else(|| {
                    panic!(
                        "JniExt::on_function: return type `{}` (encoder for `{}`) for `{}` is unresolved",
                        key,
                        ty.to_token_stream(),
                        original_ident,
                    )
                });
                let wire = entry.destination.clone();
                let conv = output_name(&lookup_ty, &wire);
                // Output wrapper takes v by value; pass __result by move.
                let wire_with_lifetime = annotate_jobject_with_lifetime(&wire, "a");
                let wire_tokens = wire_with_lifetime.to_token_stream();
                let on_err_expr: TokenStream = sentinel_for_wire(&wire);
                (
                    wire_tokens,
                    quote!(Ok(#conv(&mut env, __result)?)),
                    on_err_expr,
                )
            }
        };

    let zresult = &ext.zresult;
    let call_expr = quote!(#source_module::#original_ident(#(#call_args),*));

    let body = match &f.sig.output {
        syn::ReturnType::Default => quote! {
            {
                (|| -> #zresult<()> {
                    #(#prelude)*
                    #call_expr;
                    #wrap_ok
                })()
                .unwrap_or_else(|err| {
                    #throw!(env, err);
                    #on_err
                })
            }
        },
        syn::ReturnType::Type(_, ty) if is_unit(ty) => quote! {
            {
                (|| -> #zresult<()> {
                    #(#prelude)*
                    #call_expr;
                    #wrap_ok
                })()
                .unwrap_or_else(|err| {
                    #throw!(env, err);
                    #on_err
                })
            }
        },
        syn::ReturnType::Type(_, ty) if is_result_of_unit(ty) => quote! {
            {
                (|| -> #zresult<()> {
                    #(#prelude)*
                    #call_expr?;
                    Ok(())
                })()
                .unwrap_or_else(|err| {
                    #throw!(env, err);
                    ()
                })
            }
        },
        syn::ReturnType::Type(_, _) => quote! {
            {
                (|| -> #zresult<#wire_return> {
                    #(#prelude)*
                    let __result = #call_expr?;
                    #wrap_ok
                })()
                .unwrap_or_else(|err| {
                    #throw!(env, err);
                    #on_err
                })
            }
        },
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

fn option_input(t1: &syn::Type, registry: &Registry) -> Option<(syn::Type, syn::Expr)> {
    let inner_entry = lookup_input_resolved(registry, t1)?;
    let inner_wire = inner_entry.destination.clone();
    let inner_conv = input_name(t1, &inner_wire);

    if is_jni_primitive(&inner_wire) {
        // Boxed primitive: receive JObject, unbox via Java method, then
        // delegate to the primitive's converter.
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
                Some(#inner_conv(env, __unboxed)?)
            } else {
                None
            }
        });
        return Some((syn::parse_quote!(jni::objects::JObject), body));
    }

    // Reference inner — pass-through .is_null() check.
    let body: syn::Expr = syn::parse_quote!({
        if !v.is_null() {
            Some(#inner_conv(env, v)?)
        } else {
            None
        }
    });
    // Use a fresh value of the inner wire type for the JObject form.
    Some((inner_wire, body))
}

fn option_output(t1: &syn::Type, registry: &Registry) -> Option<(syn::Type, syn::Expr)> {
    let inner_entry = lookup_output_resolved(registry, t1)?;
    let inner_wire = inner_entry.destination.clone();
    let inner_conv = output_name(t1, &inner_wire);

    if is_jni_primitive(&inner_wire) {
        let java_class = jni_box_class(&inner_wire);
        let box_sig = jni_box_sig(&inner_wire);
        let variant = jni_box_variant(&inner_wire);
        let variant_id = format_ident!("{}", variant);
        // v is Option<T> by value; consume via match.
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
        return Some((syn::parse_quote!(jni::objects::JObject), body));
    }

    let body: syn::Expr = syn::parse_quote!({
        match v {
            Some(value) => #inner_conv(env, value)?,
            None => jni::objects::JObject::null().into(),
        }
    });
    Some((inner_wire, body))
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
        let arg_entry = lookup_output_resolved(registry, arg_ty)?;
        let arg_wire = arg_entry.destination.clone();
        let conv = output_name(arg_ty, &arg_wire);

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
        let arg_entry = lookup_output_resolved(registry, arg_ty)?;
        let arg_wire = arg_entry.destination.clone();
        let conv = output_name(arg_ty, &arg_wire);
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
        let field_entry = lookup_input_resolved(registry, &field.ty)?;
        let field_wire = field_entry.destination.clone();
        let field_conv = input_name(&field.ty, &field_wire);

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
        let field_entry = lookup_output_resolved(registry, &field.ty)?;
        let field_wire = field_entry.destination.clone();
        let field_conv = output_name(&field.ty, &field_wire);

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

fn is_unit(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Tuple(t) if t.elems.is_empty())
}

fn is_result_of_unit(ty: &syn::Type) -> bool {
    if let syn::Type::Path(tp) = ty {
        if let Some(last) = tp.path.segments.last() {
            if last.ident == "ZResult" {
                if let syn::PathArguments::AngleBracketed(ab) = &last.arguments {
                    if ab.args.len() == 1 {
                        if let Some(syn::GenericArgument::Type(inner)) = ab.args.first() {
                            return is_unit(inner);
                        }
                    }
                }
            }
        }
    }
    false
}

fn lookup_input_resolved<'a>(
    registry: &'a Registry,
    ty: &syn::Type,
) -> Option<&'a crate::core::registry::TypeEntry> {
    let key = TypeKey::from_type(ty);
    lookup_input_resolved_by_key(registry, &key)
}

fn lookup_output_resolved<'a>(
    registry: &'a Registry,
    ty: &syn::Type,
) -> Option<&'a crate::core::registry::TypeEntry> {
    let key = TypeKey::from_type(ty);
    lookup_output_resolved_by_key(registry, &key)
}

fn lookup_input_resolved_by_key<'a>(
    registry: &'a Registry,
    key: &TypeKey,
) -> Option<&'a crate::core::registry::TypeEntry> {
    for bucket in &registry.input_types {
        if let Some(slot) = bucket.get(key) {
            return slot.as_ref();
        }
    }
    None
}

fn lookup_output_resolved_by_key<'a>(
    registry: &'a Registry,
    key: &TypeKey,
) -> Option<&'a crate::core::registry::TypeEntry> {
    for bucket in &registry.output_types {
        if let Some(slot) = bucket.get(key) {
            return slot.as_ref();
        }
    }
    None
}
