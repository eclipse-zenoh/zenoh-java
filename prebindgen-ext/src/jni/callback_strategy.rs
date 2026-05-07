//! Auto-generation of per-signature `process_kotlin_<Stem>_callback`
//! Rust closures and matching Kotlin `fun interface` files for every
//! `impl Fn(T1, T2, …) + Send + Sync + 'static` parameter type seen
//! in `#[prebindgen]` function signatures.
//!
//! The strategy
//! 1. Walks `syn::Item::Fn`s, finds parameter types whose canonical token
//!    stream begins with `impl Fn (` and ends with
//!    `+ Send + Sync + 'static`, deduplicated by canonical key.
//! 2. For each unique signature it
//!    - emits a Rust fn `process_kotlin_<Stem>_callback(env, callback) ->
//!      ZResult<impl Fn(T1, T2, …) + Send + Sync + 'static>`,
//!    - registers a [`TypeBinding`] for the canonical signature with wire
//!      `jni::objects::JObject` and a decoder pointing at the new fn,
//!    - writes `JNI<Stem>Callback.kt` to the configured output dir, and
//!    - records `<canonical signature> → <Kotlin FQN>` in the
//!      [`KotlinTypeMap`] so subsequent generators can resolve callback
//!      parameter types as Kotlin function-interface names.
//!
//! The `process_*` body mirrors the prior hand-written
//! `process_kotlin_sample_callback` (capture `Arc<JavaVM>` + `GlobalRef`,
//! attach thread inside the closure, encode each arg via its registered
//! `OutputFn`, call `callback.run(...)` with the assembled JNI signature).

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::path::PathBuf;

use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};

use prebindgen::SourceLocation;

use crate::core::inline_fn::OutputFn;
use crate::core::type_binding::canon_type;
use crate::core::type_registry::TypeRegistry;
use crate::jni::inline_fn_helpers::env_ref_mut_decode;
use crate::jni::wire_access::jni_field_access;
use crate::kotlin::type_map::KotlinTypeMap;

/// Names of the helpers the generated `process_kotlin_*_callback` body
/// expects to find in scope. The defaults match the existing zenoh-jni
/// helpers in `crate::utils`. Override via the builder for crates that
/// expose them under a different path.
#[derive(Clone)]
pub struct CallbackHelpers {
    pub zresult: syn::Path,
}

impl Default for CallbackHelpers {
    fn default() -> Self {
        Self {
            zresult: syn::parse_str("crate::errors::ZResult").unwrap(),
        }
    }
}

/// Builder for [`CallbacksConverter`].
pub struct CallbacksBuilder {
    aliases: HashMap<String, String>,
    kotlin_package: String,
    kotlin_output_dir: PathBuf,
    types: TypeRegistry,
    kotlin_types: KotlinTypeMap,
    helpers: CallbackHelpers,
}

impl Default for CallbacksBuilder {
    fn default() -> Self {
        Self {
            aliases: HashMap::new(),
            kotlin_package: String::new(),
            kotlin_output_dir: PathBuf::new(),
            types: TypeRegistry::new(),
            kotlin_types: KotlinTypeMap::new(),
            helpers: CallbackHelpers::default(),
        }
    }
}

impl CallbacksBuilder {
    /// Override the auto-derived stem for a specific callback signature so
    /// the generated symbols / Kotlin file land at fixed names. The stem
    /// `Subscriber` produces `process_kotlin_Subscriber_callback` and
    /// `JNISubscriberCallback.kt`.
    pub fn alias(
        mut self,
        rust_signature: impl AsRef<str>,
        stem: impl Into<String>,
    ) -> Self {
        self.aliases
            .insert(canon_type(rust_signature.as_ref()), stem.into());
        self
    }

    pub fn kotlin_package(mut self, p: impl Into<String>) -> Self {
        self.kotlin_package = p.into();
        self
    }

    pub fn kotlin_output_dir(mut self, p: impl Into<PathBuf>) -> Self {
        self.kotlin_output_dir = p.into();
        self
    }

    pub fn type_registry(mut self, t: TypeRegistry) -> Self {
        self.types = self.types.merge(t);
        self
    }

    pub fn kotlin_types(mut self, k: KotlinTypeMap) -> Self {
        self.kotlin_types.map.extend(k.map);
        self
    }

    pub fn helpers(mut self, h: CallbackHelpers) -> Self {
        self.helpers = h;
        self
    }

    pub fn build(self) -> CallbacksConverter {
        CallbacksConverter {
            cfg: self,
            pending: VecDeque::new(),
            seen: HashSet::new(),
            buffered: false,
        }
    }
}

/// Auto-generates per-signature JNI callback wrappers.
pub struct CallbacksConverter {
    cfg: CallbacksBuilder,
    pending: VecDeque<(syn::Item, SourceLocation)>,
    seen: HashSet<String>,
    buffered: bool,
}

impl CallbacksConverter {
    pub fn builder() -> CallbacksBuilder {
        CallbacksBuilder::default()
    }

    /// Drain the source iterator on first call, scanning each function
    /// signature for unique `impl Fn(...)` parameter types and emitting a
    /// generated `process_kotlin_*_callback` Rust fn per signature.
    pub fn call<I>(&mut self, iter: &mut I) -> Option<(syn::Item, SourceLocation)>
    where
        I: Iterator<Item = (syn::Item, SourceLocation)>,
    {
        if !self.buffered {
            self.buffered = true;
            for (item, loc) in iter.by_ref() {
                if let syn::Item::Fn(f) = &item {
                    self.scan_fn(f, &loc);
                }
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

    pub fn into_type_registry(self) -> TypeRegistry {
        self.cfg.types
    }

    pub fn into_kotlin_types(self) -> KotlinTypeMap {
        self.cfg.kotlin_types
    }

    /// Non-consuming accessor for the auto-populated Kotlin type map —
    /// useful when the caller wants to keep the converter alive (or call
    /// [`Self::into_type_registry`] later) but still feed the registered
    /// callback FQNs into a downstream `KotlinInterfaceGenerator`.
    pub fn kotlin_types(&self) -> &KotlinTypeMap {
        &self.cfg.kotlin_types
    }

    fn scan_fn(&mut self, f: &syn::ItemFn, loc: &SourceLocation) {
        for input in &f.sig.inputs {
            let syn::FnArg::Typed(pat_type) = input else {
                continue;
            };
            let Some(args) = extract_fn_trait_args(&pat_type.ty) else {
                continue;
            };
            let canon = pat_type.ty.to_token_stream().to_string();
            if self.seen.contains(&canon) {
                continue;
            }
            // Don't auto-generate over a callback signature whose
            // TypeBinding is already registered (i.e. provided manually
            // by the build script). This lets the host opt out
            // selectively — used for `Query`/`Reply`/on_close which still
            // have hand-written process_kotlin_*_callback fns.
            if self.cfg.types.types.contains_key(&canon) {
                self.seen.insert(canon);
                continue;
            }
            self.seen.insert(canon.clone());
            self.emit_for_signature(&canon, &args, loc);
        }
    }

    fn emit_for_signature(
        &mut self,
        canon: &str,
        args: &[syn::Type],
        loc: &SourceLocation,
    ) {
        let stem = self.derive_stem(canon, args);
        let process_fn_ident = format_ident!("process_kotlin_{}_callback", stem);
        let kotlin_class_name = format!("JNI{}Callback", stem);
        let kotlin_fqn = if self.cfg.kotlin_package.is_empty() {
            kotlin_class_name.clone()
        } else {
            format!("{}.{}", self.cfg.kotlin_package, kotlin_class_name)
        };

        // Per-arg info: encode call, JNI sig chunk, JValue construction.
        let mut arg_idents: Vec<syn::Ident> = Vec::new();
        let mut arg_types: Vec<syn::Type> = Vec::new();
        let mut arg_preludes: Vec<TokenStream> = Vec::new();
        let mut jvalue_exprs: Vec<TokenStream> = Vec::new();
        let mut kotlin_params: Vec<String> = Vec::new();
        let mut sig = String::from("(");
        let mut used_kotlin_fqns: BTreeSet<String> = BTreeSet::new();

        for (i, arg_ty) in args.iter().enumerate() {
            let ident = format_ident!("__arg{}", i);
            let arg_canon = arg_ty.to_token_stream().to_string();
            let binding = lookup_arg_binding(&self.cfg.types, arg_ty).unwrap_or_else(|| {
                panic!(
                    "callback arg #{i} type `{arg_canon}` (in `{canon}` at {loc}): no \
                     TypeBinding registered — register it or its wire form first",
                );
            });
            let encode = binding.encode().unwrap_or_else(|| {
                panic!(
                    "callback arg #{i} type `{arg_canon}` (in `{canon}` at {loc}): \
                     TypeBinding has no encoder — every callback arg must be encodable",
                );
            });

            let (chunk, prelude, jvalue_expr) =
                self.build_arg(i, arg_ty, &arg_canon, binding.wire_type(), encode, loc);
            sig.push_str(&chunk);
            arg_preludes.push(prelude);
            jvalue_exprs.push(jvalue_expr);

            // Kotlin parameter rendering.
            let kotlin_ty = self
                .cfg
                .kotlin_types
                .lookup(&arg_canon)
                .or_else(|| {
                    if let syn::Type::Path(tp) = arg_ty {
                        if let Some(last) = tp.path.segments.last() {
                            return self.cfg.kotlin_types.lookup(&last.ident.to_string());
                        }
                    }
                    None
                })
                .unwrap_or_else(|| {
                    panic!(
                        "callback arg #{i} type `{arg_canon}` (in `{canon}` at {loc}): \
                         no kotlin_type registered",
                    )
                })
                .to_string();
            let short = register_fqn(&kotlin_ty, &mut used_kotlin_fqns);
            let optional_suffix = if is_option_type(arg_ty) { "?" } else { "" };
            kotlin_params.push(format!("        p{i}: {short}{optional_suffix},"));

            arg_idents.push(ident);
            arg_types.push(arg_ty.clone());
        }
        sig.push_str(")V");

        // ----- Generate the Rust process function -----

        let zresult = &self.cfg.helpers.zresult;
        // Derive ZError path from ZResult path by replacing the last segment.
        let zerror_ty: syn::Path = {
            let mut p = zresult.clone();
            if let Some(last) = p.segments.last_mut() {
                last.ident = syn::Ident::new("ZError", last.ident.span());
            }
            p
        };

        let arg_pat_pairs: Vec<TokenStream> = arg_idents
            .iter()
            .zip(arg_types.iter())
            .map(|(ident, ty)| quote! { #ident: #ty })
            .collect();

        let return_arg_types: Vec<TokenStream> =
            arg_types.iter().map(|t| quote! { #t }).collect();

        let sig_lit = syn::LitStr::new(&sig, proc_macro2::Span::call_site());
        let stem_lit = syn::LitStr::new(&stem, proc_macro2::Span::call_site());

        let tokens = quote! {
            #[allow(non_snake_case, unused_mut, unused_variables, unused_imports)]
            pub(crate) unsafe fn #process_fn_ident(
                env: &mut jni::JNIEnv,
                callback: &jni::objects::JObject,
            ) -> #zresult<impl Fn(#(#return_arg_types),*) + Send + Sync + 'static> {
                use std::sync::Arc;
                let java_vm = Arc::new(env.get_java_vm()
                    .map_err(|err| #zerror_ty(format!("Unable to retrieve JVM reference: {}", err)))?);
                let callback_global_ref = env.new_global_ref(callback)
                    .map_err(|err| #zerror_ty(format!("Unable to get reference to the provided callback: {}", err)))?;

                Ok(move |#(#arg_pat_pairs),*| {
                    let _ = || -> #zresult<()> {
                        let mut env = java_vm
                            .attach_current_thread_as_daemon()
                            .map_err(|err| #zerror_ty(format!(
                                "Unable to attach thread for {} callback: {}",
                                #stem_lit,
                                err
                            )))?;
                        #(#arg_preludes)*
                        env.call_method(
                            &callback_global_ref,
                            "run",
                            #sig_lit,
                            &[#(#jvalue_exprs),*],
                        )
                        .map_err(|err| {
                            let _ = env.exception_describe();
                            #zerror_ty(err.to_string())
                        })?;
                        Ok(())
                    }()
                    .map_err(|err| tracing::error!("On {} callback error: {err}", #stem_lit));
                })
            }
        };

        let item: syn::Item =
            syn::parse2(tokens).expect("CallbacksConverter: generated fn must parse");
        self.pending.push_back((item, loc.clone()));

        // ----- Register the type binding -----

        let process_fn_path = format!("{}", process_fn_ident);
        self.cfg.types.add_type_pair_mut(canon, "jni::objects::JObject");
        self.cfg
            .types
            .add_input_conversion_function_mut(canon, env_ref_mut_decode(&process_fn_path));

        // ----- Register the Kotlin FQN -----

        self.cfg.kotlin_types =
            std::mem::take(&mut self.cfg.kotlin_types).add(canon, kotlin_fqn.clone());

        // ----- Write the Kotlin fun-interface file -----

        if !self.cfg.kotlin_output_dir.as_os_str().is_empty() {
            let path = self.cfg.kotlin_output_dir.join(format!("{kotlin_class_name}.kt"));
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let contents =
                render_kotlin_interface(&self.cfg.kotlin_package, &kotlin_class_name, &kotlin_params, &used_kotlin_fqns);
            let _ = std::fs::write(&path, contents);
        }
    }

    fn derive_stem(&self, canon: &str, args: &[syn::Type]) -> String {
        if let Some(alias) = self.cfg.aliases.get(canon) {
            return alias.clone();
        }
        let mut s = String::new();
        for a in args {
            s.push_str(&type_short_ident(a));
        }
        if s.is_empty() {
            "Empty".into()
        } else {
            s
        }
    }

    fn build_arg(
        &self,
        i: usize,
        arg_ty: &syn::Type,
        arg_canon: &str,
        wire: &syn::Type,
        encode: &OutputFn,
        loc: &SourceLocation,
    ) -> (String, TokenStream, TokenStream) {
        let raw_ident = format_ident!("__arg{}", i);
        let ref_ident = format_ident!("__arg{}_ref", i);
        let enc_ident = format_ident!("__arg{}_encoded", i);
        let obj_ident = format_ident!("__arg{}_obj", i);
        // Encoders are uniformly called with a borrow — same idiom used by
        // the struct strategy (`let __<f>_value = &value.<f>`).
        let encode_call = encode.call(Some(&ref_ident));

        let bind_ref = quote! { let #ref_ident = &#raw_ident; };

        // 1) JString / JByteArray — wrapper types that implement Into<JObject>.
        if let Some((sig, _, true)) = jni_field_access(wire) {
            let prelude = quote! {
                #bind_ref
                let #enc_ident = #encode_call;
                let #obj_ident: jni::objects::JObject = #enc_ident.into();
            };
            let jvalue = quote! { jni::objects::JValue::Object(&#obj_ident) };
            return (sig.to_string(), prelude, jvalue);
        }
        // 2) Primitive wires — encoder returns the primitive directly.
        if let Some((sig, _, false)) = jni_field_access(wire) {
            let prelude = quote! {
                #bind_ref
                let #enc_ident = #encode_call;
            };
            let jvalue = quote! { jni::objects::JValue::from(#enc_ident) };
            return (sig.to_string(), prelude, jvalue);
        }
        // 3) JObject — encoder returns raw `jni::sys::jobject`. Build the
        //    JNI sig from the registered Kotlin FQN.
        if is_jobject_wire(wire) {
            let fqn = self
                .cfg
                .kotlin_types
                .lookup(arg_canon)
                .or_else(|| {
                    if let syn::Type::Path(tp) = arg_ty {
                        if let Some(last) = tp.path.segments.last() {
                            return self.cfg.kotlin_types.lookup(&last.ident.to_string());
                        }
                    }
                    None
                })
                .unwrap_or_else(|| {
                    panic!(
                        "callback arg `{arg_canon}` at {loc}: wire is JObject but no \
                         kotlin_type FQN registered to derive the JNI signature",
                    )
                });
            let internal = fqn.replace('.', "/");
            let sig = format!("L{internal};");
            // The auto-generated `encode_<Name>` takes the value by *move*,
            // not by reference, so for JObject-shaped args we feed the
            // owned value. The borrow is unused here.
            let owned_call = encode.call(Some(&raw_ident));
            let prelude = quote! {
                let #enc_ident = #owned_call;
                let #obj_ident: jni::objects::JObject =
                    unsafe { jni::objects::JObject::from_raw(#enc_ident) };
            };
            let jvalue = quote! { jni::objects::JValue::Object(&#obj_ident) };
            return (sig, prelude, jvalue);
        }
        panic!(
            "callback arg `{arg_canon}` at {loc}: unsupported wire form `{}`",
            wire.to_token_stream()
        );
    }
}

fn render_kotlin_interface(
    package: &str,
    class_name: &str,
    params: &[String],
    used_fqns: &BTreeSet<String>,
) -> String {
    let mut imports: Vec<String> = used_fqns
        .iter()
        .filter(|fqn| {
            let pkg = fqn.rsplit_once('.').map(|(p, _)| p).unwrap_or("");
            !pkg.is_empty() && pkg != package
        })
        .cloned()
        .collect();
    imports.sort();
    imports.dedup();

    let mut out = String::new();
    out.push_str("// Auto-generated by CallbacksConverter — do not edit by hand.\n");
    if !package.is_empty() {
        out.push_str(&format!("package {}\n\n", package));
    }
    for imp in &imports {
        out.push_str(&format!("import {}\n", imp));
    }
    if !imports.is_empty() {
        out.push('\n');
    }
    out.push_str(&format!("public fun interface {} {{\n", class_name));
    if params.is_empty() {
        out.push_str("    fun run()\n");
    } else {
        out.push_str("    fun run(\n");
        for p in params {
            out.push_str(p);
            out.push('\n');
        }
        out.push_str("    )\n");
    }
    out.push_str("}\n");
    out
}

/// True iff `ty` parses as `impl Fn(...) + Send + Sync + 'static` and
/// returns the `Fn` argument-types in declaration order.
fn extract_fn_trait_args(ty: &syn::Type) -> Option<Vec<syn::Type>> {
    let syn::Type::ImplTrait(it) = ty else {
        return None;
    };
    let mut args: Option<Vec<syn::Type>> = None;
    let mut has_send = false;
    let mut has_sync = false;
    let mut has_static = false;
    for bound in &it.bounds {
        match bound {
            syn::TypeParamBound::Trait(tb) => {
                let last = tb.path.segments.last()?;
                let name = last.ident.to_string();
                match name.as_str() {
                    "Fn" => {
                        let syn::PathArguments::Parenthesized(p) = &last.arguments else {
                            return None;
                        };
                        args = Some(p.inputs.iter().cloned().collect());
                    }
                    "Send" => has_send = true,
                    "Sync" => has_sync = true,
                    _ => return None,
                }
            }
            syn::TypeParamBound::Lifetime(lt) if lt.ident == "static" => has_static = true,
            _ => return None,
        }
    }
    if has_send && has_sync && has_static {
        args
    } else {
        None
    }
}

/// Look up a callback-arg type in the registry — bare path-tail first,
/// canonical key as fallback. Mirrors `lookup_field_binding` from the
/// struct strategy.
fn lookup_arg_binding(
    registry: &TypeRegistry,
    ty: &syn::Type,
) -> Option<crate::core::type_binding::TypeBinding> {
    if let syn::Type::Path(tp) = ty {
        if let Some(last) = tp.path.segments.last() {
            if let Some(b) = registry.get_binding(&last.ident.to_string()) {
                return Some(b);
            }
        }
    }
    let key = canon_type(&ty.to_token_stream().to_string());
    registry.get_binding(&key)
}

fn is_jobject_wire(wire: &syn::Type) -> bool {
    if let syn::Type::Path(tp) = wire {
        if let Some(last) = tp.path.segments.last() {
            return last.ident == "JObject";
        }
    }
    false
}

fn type_short_ident(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(tp) => {
            if let Some(last) = tp.path.segments.last() {
                return last.ident.to_string();
            }
            "Unknown".into()
        }
        _ => "Unknown".into(),
    }
}

fn is_option_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(tp) = ty {
        if let Some(last) = tp.path.segments.last() {
            return last.ident == "Option";
        }
    }
    false
}

fn register_fqn(fqn: &str, used: &mut BTreeSet<String>) -> String {
    if fqn.contains('.') {
        used.insert(fqn.to_string());
        fqn.rsplit('.').next().unwrap_or(fqn).to_string()
    } else {
        fqn.to_string()
    }
}
