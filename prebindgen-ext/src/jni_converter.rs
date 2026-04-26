//! JNI binding generators for items marked with `#[prebindgen]`.
//!
//! Two converters split the work along the natural axis:
//!
//! * [`JniStructConverter`] consumes `#[prebindgen]` `syn::ItemStruct`s,
//!   emits a JNI decoder + a Kotlin `data class`, and inserts an
//!   auto-generated [`TypeBinding`] into a shared [`JniTypeBinding`].
//! * [`JniMethodsConverter`] consumes `#[prebindgen]` `syn::ItemFn`s,
//!   classifies each argument/return against the (now fully populated)
//!   [`JniTypeBinding`], and emits a `Java_<class>_<name>ViaJNI` wrapper
//!   plus a matching Kotlin `external fun`.
//!
//! # Type registry
//!
//! Every Rust type-shape that appears in a `#[prebindgen]` function's
//! signature must have an explicit row in the [`JniTypeBinding`] registry,
//! keyed by the canonical `to_token_stream()` form of the type. There are
//! no implicit fallbacks: missing row ⇒ panic with a clear "register `<key>`"
//! message.
//!
//! Built-in rows for `bool` and `Duration` are pre-registered by
//! [`JniTypeBinding::with_builtins`] (applied automatically inside
//! [`MethodsBuilder::default`]).

use std::collections::{BTreeSet, VecDeque};
use std::path::PathBuf;

use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};

use prebindgen::SourceLocation;

use crate::jni_type_binding::{JniTypeBinding, ReturnEncode};
pub use crate::jni_type_binding::{InlineFn, TypeBinding};

// =====================================================================
// JniStructConverter
// =====================================================================

/// Builder for [`JniStructConverter`].
pub struct StructBuilder {
    /// Module path used to fully-qualify the struct type in the generated
    /// `decode_<Name>` function (e.g. `"zenoh_flat::ext"`).
    source_module: syn::Path,
    /// `ZResult` type used in the decoder return signature.
    zresult: syn::Path,
    /// Type registry that the struct converter mutates as it processes each
    /// `#[prebindgen]` struct.
    types: JniTypeBinding,
}

impl Default for StructBuilder {
    fn default() -> Self {
        Self {
            source_module: syn::parse_str("crate").unwrap(),
            zresult: syn::parse_str("ZResult").unwrap(),
            types: JniTypeBinding::new().with_builtins(),
        }
    }
}

impl StructBuilder {
    /// Module path that contains the `#[prebindgen]` struct types
    /// (e.g. `"zenoh_flat::ext"`).
    pub fn source_module(mut self, path: impl AsRef<str>) -> Self {
        self.source_module = syn::parse_str(path.as_ref()).expect("invalid source_module path");
        self
    }

    /// Path of the `ZResult` type used in the decoder's return type.
    pub fn zresult(mut self, path: impl AsRef<str>) -> Self {
        self.zresult = syn::parse_str(path.as_ref()).expect("invalid zresult path");
        self
    }

    /// Register or replace a single [`TypeBinding`] in the type registry.
    pub fn type_binding(mut self, binding: TypeBinding) -> Self {
        self.types.types.insert(binding.name().to_string(), binding);
        self
    }

    /// Merge a reusable [`JniTypeBinding`] into the type registry.
    pub fn jni_type_binding(mut self, bindings: JniTypeBinding) -> Self {
        self.types.types.extend(bindings.types);
        self.types
            .kotlin_data_classes
            .extend(bindings.kotlin_data_classes);
        self
    }

    pub fn build(self) -> JniStructConverter {
        JniStructConverter {
            cfg: self,
            pending: VecDeque::new(),
            buffered: false,
        }
    }
}

/// Converter that turns `#[prebindgen]`-marked Rust structs into JNI
/// decoder functions and Kotlin `data class` strings, while populating a
/// shared [`JniTypeBinding`] with one auto-generated entry per struct.
pub struct JniStructConverter {
    cfg: StructBuilder,
    pending: VecDeque<(syn::Item, SourceLocation)>,
    buffered: bool,
}

impl JniStructConverter {
    pub fn builder() -> StructBuilder {
        StructBuilder::default()
    }

    /// Drain `iter` on the first call, convert each struct item, and queue
    /// the result for the next `pop` from this batching closure.
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

    /// Closure suitable for `itertools::batching`.
    pub fn as_closure<'a, I>(
        &'a mut self,
    ) -> impl FnMut(&mut I) -> Option<(syn::Item, SourceLocation)> + 'a
    where
        I: Iterator<Item = (syn::Item, SourceLocation)>,
    {
        move |iter| self.call(iter)
    }

    /// Consume the converter and return the populated [`JniTypeBinding`].
    pub fn into_jni_type_binding(self) -> JniTypeBinding {
        self.cfg.types
    }

    fn convert(&mut self, item: syn::Item, loc: &SourceLocation) -> syn::Item {
        match item {
            syn::Item::Struct(s) => self.convert_struct(s, loc),
            other => panic!(
                "JniStructConverter received a non-struct item at {loc}: {}",
                other.to_token_stream()
            ),
        }
    }

    /// Emit a JNI decoder for a `#[prebindgen]` struct and a matching Kotlin
    /// `data class`, then auto-register a `TypeBinding` so the struct can
    /// appear by value in a wrapped function's signature.
    fn convert_struct(&mut self, s: syn::ItemStruct, loc: &SourceLocation) -> syn::Item {
        let struct_name = s.ident.to_string();
        let struct_ident = s.ident.clone();
        let decoder_ident = format_ident!("decode_{}", struct_ident);
        let zresult = self.cfg.zresult.clone();
        let struct_module = self.cfg.source_module.clone();

        let syn::Fields::Named(named) = &s.fields else {
            panic!("tuple / unit structs are not supported at {loc}");
        };

        let mut field_preludes: Vec<TokenStream> = Vec::new();
        let mut field_init: Vec<TokenStream> = Vec::new();
        let mut kotlin_field_lines: Vec<String> = Vec::new();

        for field in &named.named {
            let fname_ident = field
                .ident
                .as_ref()
                .unwrap_or_else(|| panic!("unnamed field in struct `{struct_name}` at {loc}"))
                .clone();
            let fname = fname_ident.to_string();
            let kotlin_fname = snake_to_camel(&fname);
            let err_prefix = format!("{struct_name}.{kotlin_fname}: {{}}");

            let binding = self.lookup_struct_field_binding(&field.ty).unwrap_or_else(|| {
                panic!(
                    "unsupported field type `{}` for `{}.{}` at {loc}",
                    field.ty.to_token_stream(),
                    struct_name,
                    fname
                )
            });
            let (jni_sig, jvalue_method) =
                jni_primitive_signature(binding.jni_type()).unwrap_or_else(|| {
                    panic!(
                        "field `{}.{}` at {loc}: type `{}` has non-primitive JNI wire form `{}`",
                        struct_name,
                        fname,
                        field.ty.to_token_stream(),
                        binding.jni_type().to_token_stream()
                    )
                });
            let raw_ident = format_ident!("__{}_raw", fname_ident);
            let jni_type = binding.jni_type();
            let decode_expr = binding
                .decode()
                .expect("struct-field binding must have a decode")
                .call(&raw_ident);
            field_preludes.push(quote! {
                let #raw_ident: #jni_type = env.get_field(obj, #kotlin_fname, #jni_sig)
                    .and_then(|v| v.#jvalue_method())
                    .map_err(|err| zerror!(#err_prefix, err))? as _;
                let #fname_ident = #decode_expr;
            });
            field_init.push(quote! { #fname_ident });
            kotlin_field_lines.push(format!(
                "    val {}: {},",
                kotlin_fname,
                binding.kotlin_type()
            ));
        }

        let tokens = quote! {
            #[allow(non_snake_case, unused_mut, unused_variables)]
            pub(crate) fn #decoder_ident(
                env: &mut jni::JNIEnv,
                obj: &jni::objects::JObject,
            ) -> #zresult<#struct_module::#struct_ident> {
                #(#field_preludes)*
                Ok(#struct_module::#struct_ident {
                    #(#field_init),*
                })
            }
        };

        let decoder_path = format!("decode_{struct_name}");
        let row = TypeBinding::param(
            &struct_name,
            &struct_name,
            "jni::objects::JObject",
            InlineFn::env_ref_mut(&decoder_path),
        );
        self.cfg.types.types.insert(row.name().to_string(), row);

        let block = format!(
            "data class {}(\n{}\n)",
            struct_name,
            kotlin_field_lines.join("\n")
        );
        self.cfg.types.kotlin_data_classes.push(block);

        syn::parse2(tokens).expect("generated struct decoder must parse")
    }

    /// Look up a `#[prebindgen]` struct field's type in the registry. Fields
    /// must use the type's bare path-tail name (e.g. `bool`, `i64`,
    /// `CongestionControl`) and must resolve to a registered binding whose
    /// JNI wire form is one of the primitive `j*` types.
    fn lookup_struct_field_binding(&self, ty: &syn::Type) -> Option<&TypeBinding> {
        let syn::Type::Path(tp) = ty else { return None };
        let last = tp.path.segments.last()?;
        let name = last.ident.to_string();
        self.cfg.types.types.get(&name)
    }
}

// =====================================================================
// JniMethodsConverter
// =====================================================================

/// Builder for [`JniMethodsConverter`].
pub struct MethodsBuilder {
    class_prefix: String,
    function_suffix: String,
    source_module: syn::Path,
    zresult: syn::Path,
    throw_exception: syn::Path,
    types: JniTypeBinding,
    kotlin: Option<KotlinConfig>,
}

/// Settings for generating a companion Kotlin file with `external fun`
/// prototypes. Enabled via [`MethodsBuilder::kotlin_output`].
pub(crate) struct KotlinConfig {
    output_path: PathBuf,
    package: String,
    class_name: String,
    throws_class_fqn: Option<String>,
    init_load_fqn: Option<String>,
}

impl Default for MethodsBuilder {
    fn default() -> Self {
        Self {
            class_prefix: String::new(),
            function_suffix: String::new(),
            source_module: syn::parse_str("crate").unwrap(),
            zresult: syn::parse_str("ZResult").unwrap(),
            throw_exception: syn::parse_str("throw_exception").unwrap(),
            types: JniTypeBinding::new().with_builtins(),
            kotlin: None,
        }
    }
}

impl MethodsBuilder {
    /// JNI class prefix prepended to each function name.
    pub fn class_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.class_prefix = prefix.into();
        self
    }

    /// Suffix appended to the camel-case function name.
    pub fn function_suffix(mut self, suffix: impl Into<String>) -> Self {
        self.function_suffix = suffix.into();
        self
    }

    /// Fully-qualified path of the module that contains the original Rust
    /// functions being wrapped.
    pub fn source_module(mut self, path: impl AsRef<str>) -> Self {
        self.source_module = syn::parse_str(path.as_ref()).expect("invalid source_module path");
        self
    }

    /// Path of the `ZResult` type used in the closure's return type.
    pub fn zresult(mut self, path: impl AsRef<str>) -> Self {
        self.zresult = syn::parse_str(path.as_ref()).expect("invalid zresult path");
        self
    }

    /// Path of the `throw_exception!` macro.
    pub fn throw_exception(mut self, path: impl AsRef<str>) -> Self {
        self.throw_exception =
            syn::parse_str(path.as_ref()).expect("invalid throw_exception path");
        self
    }

    /// Register or replace a [`TypeBinding`] by name.
    pub fn type_binding(mut self, binding: TypeBinding) -> Self {
        self.types.types.insert(binding.name().to_string(), binding);
        self
    }

    /// Merge a reusable [`JniTypeBinding`] into the type registry.
    pub fn jni_type_binding(mut self, bindings: JniTypeBinding) -> Self {
        self.types.types.extend(bindings.types);
        self.types
            .kotlin_data_classes
            .extend(bindings.kotlin_data_classes);
        self
    }

    /// Enable Kotlin-side prototype generation.
    pub fn kotlin_output(mut self, path: impl Into<PathBuf>) -> Self {
        self.kotlin
            .get_or_insert_with(KotlinConfig::default)
            .output_path = path.into();
        self
    }

    /// Kotlin `package` of the generated file.
    pub fn kotlin_package(mut self, pkg: impl Into<String>) -> Self {
        self.kotlin.get_or_insert_with(KotlinConfig::default).package = pkg.into();
        self
    }

    /// Name of the generated Kotlin `object`.
    pub fn kotlin_class(mut self, name: impl Into<String>) -> Self {
        self.kotlin
            .get_or_insert_with(KotlinConfig::default)
            .class_name = name.into();
        self
    }

    /// FQN of the exception type to annotate every `external fun` with via
    /// `@Throws(<last>::class)`.
    pub fn kotlin_throws(mut self, fqn: impl Into<String>) -> Self {
        self.kotlin
            .get_or_insert_with(KotlinConfig::default)
            .throws_class_fqn = Some(fqn.into());
        self
    }

    /// FQN of a singleton referenced from the generated `init { ... }` block.
    pub fn kotlin_init(mut self, fqn: impl Into<String>) -> Self {
        self.kotlin
            .get_or_insert_with(KotlinConfig::default)
            .init_load_fqn = Some(fqn.into());
        self
    }

    pub fn build(self) -> JniMethodsConverter {
        JniMethodsConverter {
            cfg: self,
            pending: VecDeque::new(),
            buffered: false,
            kotlin_funs: Vec::new(),
            kotlin_used_fqns: BTreeSet::new(),
        }
    }
}

impl Default for KotlinConfig {
    fn default() -> Self {
        Self {
            output_path: PathBuf::new(),
            package: String::new(),
            class_name: String::new(),
            throws_class_fqn: None,
            init_load_fqn: None,
        }
    }
}

/// Converter that transforms `#[prebindgen]`-marked Rust functions into JNI
/// `Java_*` wrappers and matching Kotlin `external fun` prototypes.
pub struct JniMethodsConverter {
    cfg: MethodsBuilder,
    pending: VecDeque<(syn::Item, SourceLocation)>,
    buffered: bool,
    kotlin_funs: Vec<String>,
    kotlin_used_fqns: BTreeSet<String>,
}

impl JniMethodsConverter {
    pub fn builder() -> MethodsBuilder {
        MethodsBuilder::default()
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

    /// Write the accumulated Kotlin file to the configured output path.
    /// No-op when Kotlin output was not enabled.
    pub fn write_kotlin(&self) -> std::io::Result<()> {
        let Some(kt) = self.cfg.kotlin.as_ref() else {
            return Ok(());
        };
        if kt.output_path.as_os_str().is_empty() {
            return Ok(());
        }
        let contents = self.render_kotlin(kt);
        if let Some(parent) = kt.output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&kt.output_path, contents)
    }

    fn render_kotlin(&self, kt: &KotlinConfig) -> String {
        let mut used = self.kotlin_used_fqns.clone();
        if let Some(fqn) = kt.init_load_fqn.as_ref() {
            if fqn.contains('.') {
                used.insert(fqn.clone());
            }
        }

        let mut imports: Vec<String> = used
            .into_iter()
            .filter(|fqn| {
                let pkg = fqn.rsplit_once('.').map(|(p, _)| p).unwrap_or("");
                !pkg.is_empty() && pkg != kt.package
            })
            .collect();
        imports.sort();
        imports.dedup();

        let mut out = String::new();
        out.push_str("// Auto-generated by JniConverter — do not edit by hand.\n");
        if !kt.package.is_empty() {
            out.push_str(&format!("package {}\n\n", kt.package));
        }
        for imp in &imports {
            out.push_str(&format!("import {}\n", imp));
        }
        if !imports.is_empty() {
            out.push('\n');
        }
        for block in &self.cfg.types.kotlin_data_classes {
            out.push_str(block);
            out.push_str("\n\n");
        }
        out.push_str(&format!("internal object {} {{\n", kt.class_name));
        if let Some(fqn) = kt.init_load_fqn.as_ref() {
            let short = fqn.rsplit('.').next().unwrap_or(fqn);
            out.push_str(&format!("    init {{ {} }}\n\n", short));
        }
        for (i, block) in self.kotlin_funs.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.push_str(block);
            out.push('\n');
        }
        out.push_str("}\n");
        out
    }

    fn convert(&mut self, item: syn::Item, loc: &SourceLocation) -> syn::Item {
        match item {
            syn::Item::Fn(func) => syn::Item::Fn(self.convert_fn(func, loc)),
            other => panic!(
                "JniMethodsConverter received a non-fn item at {loc}: {}",
                other.to_token_stream()
            ),
        }
    }

    fn convert_fn(&mut self, func: syn::ItemFn, loc: &SourceLocation) -> syn::ItemFn {
        let original_name = func.sig.ident.to_string();
        let camel = snake_to_camel(&original_name);
        let jni_name = format_ident!(
            "{}{}{}",
            self.cfg.class_prefix,
            camel,
            self.cfg.function_suffix
        );
        let orig_ident = &func.sig.ident;
        let source_module = self.cfg.source_module.clone();
        let zresult = self.cfg.zresult.clone();
        let throw_exception = self.cfg.throw_exception.clone();

        let mut prelude: Vec<TokenStream> = Vec::new();
        let mut jni_params: Vec<TokenStream> = Vec::new();
        let mut call_args: Vec<TokenStream> = Vec::new();
        let mut kotlin_params: Vec<String> = Vec::new();
        let mut local_kotlin_fqns: BTreeSet<String> = BTreeSet::new();
        let kt_enabled = self.cfg.kotlin.is_some();

        for input in &func.sig.inputs {
            let syn::FnArg::Typed(pat_type) = input else {
                panic!("receiver args not supported at {loc}");
            };
            let syn::Pat::Ident(pat_ident) = &*pat_type.pat else {
                panic!("non-ident param pattern at {loc}");
            };
            let name = &pat_ident.ident;
            let ty = &*pat_type.ty;

            self.emit_arg(
                name,
                ty,
                loc,
                &mut prelude,
                &mut jni_params,
                &mut call_args,
                &mut kotlin_params,
                &mut local_kotlin_fqns,
                kt_enabled,
            );
        }

        // Return type.
        let mut kotlin_ret: Option<String> = None;
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
                    let key = ty.to_token_stream().to_string();
                    let binding = self.cfg.types.types.get(&key).unwrap_or_else(|| {
                        panic!(
                            "unsupported return type `{}` for `{}` at {loc}: \
                             register a TypeBinding keyed `{}`",
                            ty.to_token_stream(),
                            original_name,
                            key
                        )
                    });
                    let encode = binding.encode().unwrap_or_else(|| {
                        panic!(
                            "TypeBinding `{}` has no encode (return direction) at {loc}",
                            key
                        )
                    });
                    let default_expr = binding
                        .default_expr()
                        .expect("encode-bearing row must have default_expr");
                    let jni_type = binding.jni_type();

                    if kt_enabled {
                        let kt = binding.kotlin_type();
                        let short = kotlin_register_fqn(kt, &mut local_kotlin_fqns);
                        kotlin_ret = Some(short);
                    }

                    let wrap_ok_ts = match encode {
                        ReturnEncode::Wrapper(p) => quote! { #p(&mut env, __result) },
                        ReturnEncode::ArcIntoRaw => quote! {
                            Ok(std::sync::Arc::into_raw(std::sync::Arc::new(__result)))
                        },
                    };

                    (
                        quote! { #jni_type },
                        wrap_ok_ts,
                        quote! { #default_expr },
                        quote! { #zresult<#jni_type> },
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

        // Assemble the Kotlin `external fun ...` block.
        if kt_enabled {
            let kt_fn_name = format!("{}{}", camel, self.cfg.function_suffix);
            let ret_suffix = match &kotlin_ret {
                Some(r) => format!(": {}", r),
                None => String::new(),
            };
            let params_joined = if kotlin_params.is_empty() {
                String::new()
            } else {
                format!("\n        {},\n    ", kotlin_params.join(",\n        "))
            };
            let throws_line = if self
                .cfg
                .kotlin
                .as_ref()
                .and_then(|k| k.throws_class_fqn.as_ref())
                .is_some()
            {
                let fqn = self
                    .cfg
                    .kotlin
                    .as_ref()
                    .unwrap()
                    .throws_class_fqn
                    .as_ref()
                    .unwrap();
                let short = fqn.rsplit('.').next().unwrap_or(fqn);
                format!("@Throws({}::class)\n    ", short)
            } else {
                String::new()
            };
            let block = format!(
                "    @JvmStatic\n    {}external fun {}({}){}",
                throws_line, kt_fn_name, params_joined, ret_suffix
            );
            self.kotlin_funs.push(block);
            self.kotlin_used_fqns.extend(local_kotlin_fqns);
            if let Some(kt) = self.cfg.kotlin.as_ref() {
                if let Some(fqn) = kt.throws_class_fqn.as_ref() {
                    if fqn.contains('.') {
                        self.kotlin_used_fqns.insert(fqn.clone());
                    }
                }
            }
        }

        syn::parse2(tokens).expect("generated JNI wrapper must parse")
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_arg(
        &self,
        name: &syn::Ident,
        ty: &syn::Type,
        loc: &SourceLocation,
        prelude: &mut Vec<TokenStream>,
        jni_params: &mut Vec<TokenStream>,
        call_args: &mut Vec<TokenStream>,
        kotlin_params: &mut Vec<String>,
        local_kotlin_fqns: &mut BTreeSet<String>,
        kt_enabled: bool,
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
        let decode = binding.decode().unwrap_or_else(|| {
            panic!(
                "TypeBinding `{}` has no decode (param direction) at {loc}",
                key
            )
        });

        let pat = if binding.is_pointer() {
            format_ident!("{}_ptr", name)
        } else {
            name.clone()
        };
        let jt = binding.jni_type();
        jni_params.push(quote! { #pat: #jt });

        let expr = decode.call(&pat);
        prelude.push(quote! { let #name = #expr; });

        if binding.is_borrow() {
            call_args.push(quote! { &#name });
        } else {
            call_args.push(quote! { #name });
        }

        if kt_enabled {
            let short = kotlin_register_fqn(binding.kotlin_type(), local_kotlin_fqns);
            let suffix = if binding.is_option() { "?" } else { "" };
            kotlin_params.push(format!(
                "{}: {}{}",
                kotlin_param_name(&name.to_string(), binding.is_pointer()),
                short,
                suffix
            ));
        }
    }
}

// =====================================================================
// Internal helpers
// =====================================================================

/// Map a primitive JNI wire type (`jni::sys::j*`) to the JVM field
/// signature character and the matching `JValue` accessor method.
/// Returns `None` for non-primitive (object-shaped) wire types.
fn jni_primitive_signature(jni_type: &syn::Type) -> Option<(&'static str, syn::Ident)> {
    let syn::Type::Path(tp) = jni_type else {
        return None;
    };
    let last = tp.path.segments.last()?;
    let (sig, accessor) = match last.ident.to_string().as_str() {
        "jboolean" => ("Z", "z"),
        "jbyte" => ("B", "b"),
        "jchar" => ("C", "c"),
        "jshort" => ("S", "s"),
        "jint" => ("I", "i"),
        "jlong" => ("J", "j"),
        "jfloat" => ("F", "f"),
        "jdouble" => ("D", "d"),
        _ => return None,
    };
    Some((sig, format_ident!("{}", accessor)))
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

/// Map a Rust snake_case arg name to its Kotlin camelCase form, appending
/// `"Ptr"` for raw-pointer slots.
fn kotlin_param_name(rust_name: &str, is_pointer: bool) -> String {
    let base = snake_to_camel(rust_name);
    if is_pointer {
        format!("{}Ptr", base)
    } else {
        base
    }
}

/// Record `fqn` in `used` if it looks fully-qualified (contains `.`) and
/// return the short name used at the emission site.
fn kotlin_register_fqn(fqn: &str, used: &mut BTreeSet<String>) -> String {
    if fqn.contains('.') {
        used.insert(fqn.to_string());
        fqn.rsplit('.').next().unwrap_or(fqn).to_string()
    } else {
        fqn.to_string()
    }
}
