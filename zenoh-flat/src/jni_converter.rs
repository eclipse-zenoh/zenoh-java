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
//!     .struct_decoder("KeyExpr", "crate::key_expr::decode_jni_key_expr")
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

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::path::PathBuf;

use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};

use prebindgen::SourceLocation;

/// Builder for [`JniConverter`].
pub struct Builder {
    class_prefix: String,
    function_suffix: String,
    source_module: syn::Path,
    /// Module path where `#[prebindgen]` struct types are declared. Used to
    /// fully-qualify the struct name in the auto-generated decoder's return
    /// type and constructor. Defaults to `source_module` when unset.
    struct_source_module: Option<syn::Path>,
    owned_object: syn::Path,
    zresult: syn::Path,
    throw_exception: syn::Path,
    string_decoder: Option<syn::Path>,
    byte_array_decoder: Option<syn::Path>,
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
    /// Kotlin output config — if `None`, no Kotlin file is emitted.
    kotlin: Option<KotlinConfig>,
}

/// Settings for generating a companion Kotlin file with `external fun`
/// prototypes. Enabled via [`Builder::kotlin_output`]. Per-type Kotlin names
/// are stored alongside each Rust decoder registration (see
/// [`Builder::struct_decoder`], [`Builder::callback_decoder`], and the return
/// wrappers), so one call registers both sides.
pub(crate) struct KotlinConfig {
    output_path: PathBuf,
    package: String,
    class_name: String,
    /// FQN of the `@Throws(<last>::class)` exception — `None` disables the
    /// annotation.
    throws_class_fqn: Option<String>,
    /// FQN of a singleton referenced inside the generated `init { ... }` block
    /// to force native-library loading — `None` disables the `init`.
    init_load_fqn: Option<String>,
    /// FQN of the on-close callback type (typically
    /// `io.zenoh.jni.callbacks.JNIOnCloseCallback`). Used for the synthetic
    /// `<name>OnClose` parameter injected for every callback arg.
    on_close_callback_fqn: String,
    /// Per-source-type Kotlin names (FQN or bare) for struct-decoded args.
    struct_kotlin_types: HashMap<String, String>,
    /// Per-element-type Kotlin names for callback args (e.g. `Sample` →
    /// `io.zenoh.jni.callbacks.JNISubscriberCallback`).
    callback_kotlin_types: HashMap<String, String>,
    /// Per-return-type Kotlin names for `ZResult<T>` with a return_wrapper.
    return_kotlin_types: HashMap<String, String>,
    /// Per-element-type Kotlin names for `ZResult<Vec<T>>` with a
    /// return_wrapper_vec.
    return_kotlin_types_vec: HashMap<String, String>,
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
            struct_source_module: None,
            owned_object: syn::parse_str("OwnedObject").unwrap(),
            zresult: syn::parse_str("ZResult").unwrap(),
            throw_exception: syn::parse_str("throw_exception").unwrap(),
            string_decoder: None,
            byte_array_decoder: None,
            enum_decoders: HashMap::new(),
            callback_decoders: HashMap::new(),
            struct_decoders: HashMap::new(),
            consume_args: HashMap::new(),
            return_wrappers: HashMap::new(),
            return_wrappers_vec: HashMap::new(),
            kotlin: None,
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

    /// Fully-qualified path of the module that contains the `#[prebindgen]`
    /// struct types (e.g. `"zenoh_flat::ext"`). When unset, defaults to
    /// [`Builder::source_module`]. Used to qualify the struct type in
    /// auto-generated decoders.
    pub fn struct_source_module(mut self, path: impl AsRef<str>) -> Self {
        self.struct_source_module =
            Some(syn::parse_str(path.as_ref()).expect("invalid struct_source_module path"));
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
    ///
    /// `kotlin_type` is the Kotlin type name — FQN if out-of-package (import
    /// is auto-derived) or bare for same-package / built-in. Only read when
    /// Kotlin output is enabled via [`Builder::kotlin_output`].
    pub fn struct_decoder(
        mut self,
        type_name: impl Into<String>,
        decoder: impl AsRef<str>,
        kotlin_type: impl Into<String>,
    ) -> Self {
        let path: syn::Path =
            syn::parse_str(decoder.as_ref()).expect("invalid struct_decoder path");
        let name = type_name.into();
        let kt = kotlin_type.into();
        self.struct_decoders.insert(name.clone(), path);
        if let Some(k) = self.kotlin.as_mut() {
            k.struct_kotlin_types.insert(name, kt);
        }
        self
    }

    /// Register a decoder for an `impl Fn(T) + Send + Sync + 'static` callback
    /// parameter. `element_type_name` is the last path segment of `T`
    /// (e.g. `"Sample"`, `"Query"`, `"Reply"`). The decoder must have the
    /// signature
    /// `fn(&mut JNIEnv, JObject, JObject) -> ZResult<impl Fn(T) + Send + Sync + 'static>`.
    /// The generated JNI signature expands the single callback parameter into
    /// two JNI args: `<name>: JObject, <name>_on_close: JObject`.
    ///
    /// `kotlin_type` is the Kotlin callback type name — FQN if out-of-package,
    /// bare otherwise. Only read when Kotlin output is enabled.
    pub fn callback_decoder(
        mut self,
        element_type_name: impl Into<String>,
        decoder: impl AsRef<str>,
        kotlin_type: impl Into<String>,
    ) -> Self {
        let path: syn::Path =
            syn::parse_str(decoder.as_ref()).expect("invalid callback_decoder path");
        let name = element_type_name.into();
        let kt = kotlin_type.into();
        self.callback_decoders.insert(name.clone(), path);
        if let Some(k) = self.kotlin.as_mut() {
            k.callback_kotlin_types.insert(name, kt);
        }
        self
    }

    /// Register a return-type wrapper for `ZResult<T>` where `T`'s
    /// last-segment name equals `type_name`. `jni_type` is the generated
    /// `extern "C"` return type. `wrap_fn` must have signature
    /// `fn(&mut JNIEnv, T) -> ZResult<jni_type>`. `default_expr` is the value
    /// returned on error (before the exception is thrown on the JVM side).
    ///
    /// `kotlin_type` is the Kotlin return type name (FQN or bare). Only read
    /// when Kotlin output is enabled.
    pub fn return_wrapper(
        mut self,
        type_name: impl Into<String>,
        jni_type: impl AsRef<str>,
        wrap_fn: impl AsRef<str>,
        default_expr: impl AsRef<str>,
        kotlin_type: impl Into<String>,
    ) -> Self {
        let name = type_name.into();
        let kt = kotlin_type.into();
        self.return_wrappers
            .insert(name.clone(), parse_return_wrapper(jni_type, wrap_fn, default_expr));
        if let Some(k) = self.kotlin.as_mut() {
            k.return_kotlin_types.insert(name, kt);
        }
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
        kotlin_type: impl Into<String>,
    ) -> Self {
        let name = element_type_name.into();
        let kt = kotlin_type.into();
        self.return_wrappers_vec
            .insert(name.clone(), parse_return_wrapper(jni_type, wrap_fn, default_expr));
        if let Some(k) = self.kotlin.as_mut() {
            k.return_kotlin_types_vec.insert(name, kt);
        }
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

    /// Enable Kotlin-side prototype generation. `path` is where the `.kt`
    /// file will be written when [`JniConverter::write_kotlin`] is called.
    /// Calling this method is what turns on Kotlin output; all other
    /// `kotlin_*` methods are optional refinements.
    pub fn kotlin_output(mut self, path: impl Into<PathBuf>) -> Self {
        self.kotlin.get_or_insert_with(KotlinConfig::default).output_path = path.into();
        self
    }

    /// Kotlin `package` of the generated file (e.g. `"io.zenoh.jni"`).
    pub fn kotlin_package(mut self, pkg: impl Into<String>) -> Self {
        self.kotlin.get_or_insert_with(KotlinConfig::default).package = pkg.into();
        self
    }

    /// Name of the generated Kotlin `object` (e.g. `"JNISessionNative"`). Must
    /// agree with the JNI class_prefix — i.e. `class_prefix` should end with
    /// `"_<kotlin_class>_"`.
    pub fn kotlin_class(mut self, name: impl Into<String>) -> Self {
        self.kotlin.get_or_insert_with(KotlinConfig::default).class_name = name.into();
        self
    }

    /// FQN of the exception type to annotate every `external fun` with via
    /// `@Throws(<last>::class)`. Unset ⇒ no annotation.
    pub fn kotlin_throws(mut self, fqn: impl Into<String>) -> Self {
        self.kotlin.get_or_insert_with(KotlinConfig::default).throws_class_fqn = Some(fqn.into());
        self
    }

    /// FQN of a singleton referenced from the generated `init { ... }` block
    /// (typically `io.zenoh.ZenohLoad`) to force native-library loading.
    /// Unset ⇒ no `init` block.
    pub fn kotlin_init(mut self, fqn: impl Into<String>) -> Self {
        self.kotlin.get_or_insert_with(KotlinConfig::default).init_load_fqn = Some(fqn.into());
        self
    }

    /// FQN of the Kotlin on-close callback type used for the synthetic
    /// `<name>OnClose` parameter injected for each callback argument.
    pub fn kotlin_on_close(mut self, fqn: impl Into<String>) -> Self {
        self.kotlin.get_or_insert_with(KotlinConfig::default).on_close_callback_fqn = fqn.into();
        self
    }

    pub fn build(self) -> JniConverter {
        JniConverter {
            cfg: self,
            pending: VecDeque::new(),
            buffered: false,
            kotlin_funs: Vec::new(),
            kotlin_data_classes: Vec::new(),
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
            on_close_callback_fqn: String::new(),
            struct_kotlin_types: HashMap::new(),
            callback_kotlin_types: HashMap::new(),
            return_kotlin_types: HashMap::new(),
            return_kotlin_types_vec: HashMap::new(),
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
    /// `true` once the source iterator has been drained and sorted (structs
    /// before functions) so that function-arg classification can see every
    /// struct decoder the converter is going to auto-register.
    buffered: bool,
    /// Accumulated Kotlin `external fun ...` blocks, one per wrapped function.
    /// Populated by `convert_fn` when Kotlin output is enabled, consumed by
    /// [`JniConverter::write_kotlin`].
    kotlin_funs: Vec<String>,
    /// Accumulated Kotlin `data class ...` blocks, one per `#[prebindgen]`
    /// struct seen in the source stream. Emitted by `write_kotlin` BEFORE the
    /// `internal object { ... }` block so call sites in the same package can
    /// see the types.
    kotlin_data_classes: Vec<String>,
    /// Set of Kotlin FQNs referenced by the emitted externals. Used to derive
    /// the final `import` block (same-package and bare names are filtered
    /// out).
    kotlin_used_fqns: BTreeSet<String>,
}

impl JniConverter {
    pub fn builder() -> Builder {
        Builder::default()
    }

    /// Drain `iter` on the first call, sort so `#[prebindgen]` struct items
    /// are processed before functions (so function-arg classification can see
    /// every auto-registered struct decoder), then return converted items one
    /// at a time from the buffer. Returns `None` once the buffer is drained.
    pub fn call<I>(&mut self, iter: &mut I) -> Option<(syn::Item, SourceLocation)>
    where
        I: Iterator<Item = (syn::Item, SourceLocation)>,
    {
        if !self.buffered {
            self.buffered = true;
            let mut all: Vec<(syn::Item, SourceLocation)> = iter.by_ref().collect();
            // Stable sort: structs first (`false` < `true`), original order
            // preserved within each group.
            all.sort_by_key(|(it, _)| !matches!(it, syn::Item::Struct(_)));
            for (item, loc) in all {
                let converted = self.convert(item, &loc);
                self.pending.push_back((converted, loc));
            }
        }
        self.pending.pop_front()
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

    /// Borrowing closure suitable for `itertools::batching`. Unlike
    /// [`JniConverter::into_closure`], this does not consume `self`, so the
    /// converter survives the pipeline and [`JniConverter::write_kotlin`] can
    /// be called after the pipeline completes.
    pub fn as_closure<'a, I>(
        &'a mut self,
    ) -> impl FnMut(&mut I) -> Option<(syn::Item, SourceLocation)> + 'a
    where
        I: Iterator<Item = (syn::Item, SourceLocation)>,
    {
        move |iter| self.call(iter)
    }

    /// Write the accumulated Kotlin `external fun ...` prototypes to the
    /// configured output path. No-op when Kotlin output was not enabled via
    /// [`Builder::kotlin_output`]. The output file is overwritten if it
    /// already exists; parent directories are created as needed.
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
        // Fold init-block FQN into the used set (deferred to emission time so
        // it appears in imports only when the init block is emitted).
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
        for block in &self.kotlin_data_classes {
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
            syn::Item::Struct(s) => self.convert_struct(s, loc),
            other => other,
        }
    }

    fn convert_fn(&mut self, func: syn::ItemFn, loc: &SourceLocation) -> syn::ItemFn {
        let original_name = func.sig.ident.to_string();
        let camel = snake_to_camel(&original_name);
        let jni_name = format_ident!("{}{}{}", self.cfg.class_prefix, camel, self.cfg.function_suffix);
        let orig_ident = &func.sig.ident;
        let source_module = self.cfg.source_module.clone();
        let owned_object = self.cfg.owned_object.clone();
        let zresult = self.cfg.zresult.clone();
        let throw_exception = self.cfg.throw_exception.clone();
        let empty_consume_set: HashSet<String> = HashSet::new();
        let consume_set: HashSet<String> = self
            .cfg
            .consume_args
            .get(&original_name)
            .cloned()
            .unwrap_or(empty_consume_set);

        let mut prelude: Vec<TokenStream> = Vec::new();
        let mut jni_params: Vec<TokenStream> = Vec::new();
        let mut call_args: Vec<TokenStream> = Vec::new();
        // Kotlin param strings accumulated in parallel with `jni_params`. Only
        // populated when Kotlin output is enabled.
        let mut kotlin_params: Vec<String> = Vec::new();
        let mut local_kotlin_fqns: BTreeSet<String> = BTreeSet::new();
        let kt_cfg: Option<&KotlinConfig> = self.cfg.kotlin.as_ref();

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
                    if kt_cfg.is_some() {
                        kotlin_params.push(format!(
                            "{}: Long",
                            kotlin_param_name(&name.to_string(), /* ptr */ true)
                        ));
                    }
                }
                ArgKind::KeyExpr => {
                    let consumed = consume_set.contains(&name.to_string());
                    if consumed {
                        // Consume path: the declared KeyExpr is required (no
                        // string fallback). Arc::from_raw decrements the
                        // refcount at end of scope, freeing the handle once
                        // no other references remain. A cloned inner KeyExpr
                        // is passed to the callee by value.
                        let ptr_ident = format_ident!("{}_ptr", name);
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
                        // Non-consume path: single `JObject` holder
                        // (io.zenoh.jni.JNIKeyExpr) decoded via the
                        // `KeyExpr` entry in `struct_decoders`.
                        let decoder = self
                            .cfg
                            .struct_decoders
                            .get("KeyExpr")
                            .expect("struct_decoder(\"KeyExpr\", ...) not configured");
                        jni_params.push(quote! { #name: jni::objects::JObject });
                        prelude.push(quote! {
                            let #name = #decoder(&mut env, &#name)?;
                        });
                        call_args.push(quote! { #name });
                    }
                    if let Some(kt) = kt_cfg {
                        if consumed {
                            kotlin_params.push(format!(
                                "{}: Long",
                                kotlin_param_name(&name.to_string(), true)
                            ));
                        } else {
                            let fqn = kt.struct_kotlin_types.get("KeyExpr").cloned()
                                .expect("struct_decoder(\"KeyExpr\", ...) Kotlin type not configured");
                            let short = kotlin_register_fqn(&fqn, &mut local_kotlin_fqns);
                            kotlin_params.push(format!(
                                "{}: {}",
                                kotlin_param_name(&name.to_string(), false),
                                short
                            ));
                        }
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
                    if kt_cfg.is_some() {
                        kotlin_params.push(format!(
                            "{}: String",
                            kotlin_param_name(&name.to_string(), false)
                        ));
                    }
                }
                ArgKind::Enum(decoder) => {
                    jni_params.push(quote! { #name: jni::sys::jint });
                    prelude.push(quote! {
                        let #name = #decoder(#name)?;
                    });
                    call_args.push(quote! { #name });
                    if kt_cfg.is_some() {
                        kotlin_params.push(format!(
                            "{}: Int",
                            kotlin_param_name(&name.to_string(), false)
                        ));
                    }
                }
                ArgKind::Bool => {
                    jni_params.push(quote! { #name: jni::sys::jboolean });
                    prelude.push(quote! { let #name = #name != 0; });
                    call_args.push(quote! { #name });
                    if kt_cfg.is_some() {
                        kotlin_params.push(format!(
                            "{}: Boolean",
                            kotlin_param_name(&name.to_string(), false)
                        ));
                    }
                }
                ArgKind::Duration => {
                    jni_params.push(quote! { #name: jni::sys::jlong });
                    prelude.push(quote! {
                        let #name = std::time::Duration::from_millis(#name as u64);
                    });
                    call_args.push(quote! { #name });
                    if kt_cfg.is_some() {
                        kotlin_params.push(format!(
                            "{}: Long",
                            kotlin_param_name(&name.to_string(), false)
                        ));
                    }
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
                    if kt_cfg.is_some() {
                        kotlin_params.push(format!(
                            "{}: ByteArray?",
                            kotlin_param_name(&name.to_string(), false)
                        ));
                    }
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
                    if kt_cfg.is_some() {
                        kotlin_params.push(format!(
                            "{}: ByteArray",
                            kotlin_param_name(&name.to_string(), false)
                        ));
                    }
                }
                ArgKind::Callback { decoder, element_type_name } => {
                    let on_close_ident = format_ident!("{}_on_close", name);
                    jni_params.push(quote! { #name: jni::objects::JObject });
                    jni_params.push(quote! { #on_close_ident: jni::objects::JObject });
                    prelude.push(quote! {
                        let #name = #decoder(&mut env, #name, #on_close_ident)?;
                    });
                    call_args.push(quote! { #name });
                    if let Some(kt) = kt_cfg {
                        let cb_fqn = kt.callback_kotlin_types.get(&element_type_name).cloned()
                            .unwrap_or_else(|| panic!(
                                "callback_decoder({:?}, ...) Kotlin type not configured",
                                element_type_name
                            ));
                        let cb_short = kotlin_register_fqn(&cb_fqn, &mut local_kotlin_fqns);
                        let oc_short = kotlin_register_fqn(&kt.on_close_callback_fqn, &mut local_kotlin_fqns);
                        kotlin_params.push(format!(
                            "{}: {}",
                            kotlin_param_name(&name.to_string(), false),
                            cb_short
                        ));
                        kotlin_params.push(format!(
                            "{}OnClose: {}",
                            kotlin_param_name(&name.to_string(), false),
                            oc_short
                        ));
                    }
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
                    if kt_cfg.is_some() {
                        kotlin_params.push(format!(
                            "{}: String?",
                            kotlin_param_name(&name.to_string(), false)
                        ));
                    }
                }
                ArgKind::StructFromJObject { decoder, type_name } => {
                    jni_params.push(quote! { #name: jni::objects::JObject });
                    prelude.push(quote! {
                        let #name = #decoder(&mut env, &#name)?;
                    });
                    call_args.push(quote! { #name });
                    if let Some(kt) = kt_cfg {
                        let fqn = kt.struct_kotlin_types.get(&type_name).cloned()
                            .unwrap_or_else(|| panic!(
                                "struct_decoder({:?}, ...) Kotlin type not configured",
                                type_name
                            ));
                        let short = kotlin_register_fqn(&fqn, &mut local_kotlin_fqns);
                        kotlin_params.push(format!(
                            "{}: {}",
                            kotlin_param_name(&name.to_string(), false),
                            short
                        ));
                    }
                }
                ArgKind::OptionStructFromJObject { decoder, type_name } => {
                    jni_params.push(quote! { #name: jni::objects::JObject });
                    prelude.push(quote! {
                        let #name = if !#name.is_null() {
                            Some(#decoder(&mut env, &#name)?)
                        } else {
                            None
                        };
                    });
                    call_args.push(quote! { #name });
                    if let Some(kt) = kt_cfg {
                        let fqn = kt.struct_kotlin_types.get(&type_name).cloned()
                            .unwrap_or_else(|| panic!(
                                "struct_decoder({:?}, ...) Kotlin type not configured",
                                type_name
                            ));
                        let short = kotlin_register_fqn(&fqn, &mut local_kotlin_fqns);
                        kotlin_params.push(format!(
                            "{}: {}?",
                            kotlin_param_name(&name.to_string(), false),
                            short
                        ));
                    }
                }
                ArgKind::Unsupported => panic!(
                    "unsupported parameter type `{}` for `{}` at {loc}",
                    ty.to_token_stream(),
                    name
                ),
            }
        }

        // Kotlin return type (None = Unit). Computed in parallel with the Rust
        // return type so a single classification drives both outputs.
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
                } else if let Some(wrapper) = self.lookup_return_wrapper(&inner) {
                    if let Some(kt) = kt_cfg {
                        let fqn = self
                            .lookup_kotlin_return_type(&inner, kt)
                            .expect("return_wrapper(...) Kotlin type not configured");
                        let short = kotlin_register_fqn(&fqn, &mut local_kotlin_fqns);
                        kotlin_ret = Some(short);
                    }
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
                    if kt_cfg.is_some() {
                        kotlin_ret = Some("Long".to_string());
                    }
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

        // Assemble and stash the Kotlin `external fun ...` block so
        // `write_kotlin()` can emit it later.
        if self.cfg.kotlin.is_some() {
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
                // Short name already registered via kotlin_register_fqn for
                // the whole file (see write_kotlin); re-derive here.
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

    /// Emit a JNI decoder for a `#[prebindgen]` struct and a matching Kotlin
    /// `data class`. The struct item itself is NOT re-emitted into the output
    /// stream — only the decoder function is — so the original type stays
    /// solely in its home module (e.g. `zenoh_flat::ext`) and is referenced
    /// from generated code by its fully-qualified path.
    fn convert_struct(&mut self, s: syn::ItemStruct, loc: &SourceLocation) -> syn::Item {
        let struct_name = s.ident.to_string();
        let struct_ident = s.ident.clone();
        let decoder_ident = format_ident!("decode_{}", struct_ident);
        let zresult = self.cfg.zresult.clone();
        let struct_module = self
            .cfg
            .struct_source_module
            .clone()
            .unwrap_or_else(|| self.cfg.source_module.clone());

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

            let kind = self.classify_struct_field(&field.ty);
            match kind {
                StructFieldKind::Bool => {
                    field_preludes.push(quote! {
                        let #fname_ident = env.get_field(obj, #kotlin_fname, "Z")
                            .and_then(|v| v.z())
                            .map_err(|err| zerror!(#err_prefix, err))?;
                    });
                    field_init.push(quote! { #fname_ident });
                    kotlin_field_lines
                        .push(format!("    val {}: Boolean,", kotlin_fname));
                }
                StructFieldKind::I64 => {
                    field_preludes.push(quote! {
                        let #fname_ident = env.get_field(obj, #kotlin_fname, "J")
                            .and_then(|v| v.j())
                            .map_err(|err| zerror!(#err_prefix, err))?;
                    });
                    field_init.push(quote! { #fname_ident });
                    kotlin_field_lines.push(format!("    val {}: Long,", kotlin_fname));
                }
                StructFieldKind::F64 => {
                    field_preludes.push(quote! {
                        let #fname_ident = env.get_field(obj, #kotlin_fname, "D")
                            .and_then(|v| v.d())
                            .map_err(|err| zerror!(#err_prefix, err))?;
                    });
                    field_init.push(quote! { #fname_ident });
                    kotlin_field_lines.push(format!("    val {}: Double,", kotlin_fname));
                }
                StructFieldKind::Enum(decoder) => {
                    let raw_ident = format_ident!("__{}_raw", fname_ident);
                    field_preludes.push(quote! {
                        let #raw_ident = env.get_field(obj, #kotlin_fname, "I")
                            .and_then(|v| v.i())
                            .map_err(|err| zerror!(#err_prefix, err))?;
                        let #fname_ident = #decoder(#raw_ident)?;
                    });
                    field_init.push(quote! { #fname_ident });
                    kotlin_field_lines.push(format!("    val {}: Int,", kotlin_fname));
                }
                StructFieldKind::Unsupported => panic!(
                    "unsupported field type `{}` for `{}.{}` at {loc}",
                    field.ty.to_token_stream(),
                    struct_name,
                    fname
                ),
            }
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

        // Auto-register the decoder for future function-arg classification.
        // The decoder lives in the same module as the generated wrappers, so a
        // bare `syn::Path` resolves correctly at the wrapper call sites.
        let decoder_path: syn::Path = syn::parse_str(&format!("decode_{struct_name}"))
            .expect("generated decoder ident must parse as path");
        self.cfg
            .struct_decoders
            .insert(struct_name.clone(), decoder_path);
        if let Some(kt) = self.cfg.kotlin.as_mut() {
            // Same package as the generated Kotlin file → bare name, no FQN.
            kt.struct_kotlin_types
                .insert(struct_name.clone(), struct_name.clone());
        }

        // Accumulate the Kotlin data class for emission by write_kotlin.
        if self.cfg.kotlin.is_some() {
            let block = format!(
                "data class {}(\n{}\n)",
                struct_name,
                kotlin_field_lines.join("\n")
            );
            self.kotlin_data_classes.push(block);
        }

        syn::parse2(tokens).expect("generated struct decoder must parse")
    }

    /// Classify a `#[prebindgen]` struct field's type for JNI round-tripping.
    /// Scope is deliberately narrow (only what the current four configs need)
    /// — an unsupported type panics so we notice new cases at codegen time.
    fn classify_struct_field(&self, ty: &syn::Type) -> StructFieldKind {
        let syn::Type::Path(tp) = ty else {
            return StructFieldKind::Unsupported;
        };
        let Some(last) = tp.path.segments.last() else {
            return StructFieldKind::Unsupported;
        };
        let name = last.ident.to_string();
        match name.as_str() {
            "bool" => StructFieldKind::Bool,
            "i64" => StructFieldKind::I64,
            "f64" => StructFieldKind::F64,
            _ => {
                if let Some(decoder) = self.cfg.enum_decoders.get(&name) {
                    StructFieldKind::Enum(decoder.clone())
                } else {
                    StructFieldKind::Unsupported
                }
            }
        }
    }

    /// Resolve the Kotlin return-type FQN for a `ZResult<T>` inner type `T`.
    fn lookup_kotlin_return_type(&self, inner: &syn::Type, kt: &KotlinConfig) -> Option<String> {
        let syn::Type::Path(tp) = inner else { return None };
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
            return kt.return_kotlin_types_vec.get(&elem_name).cloned();
        }
        kt.return_kotlin_types.get(&name).cloned()
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
                        return ArgKind::Callback {
                            decoder: decoder.clone(),
                            element_type_name: elem,
                        };
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
                if name == "Option" && is_option_of_vec_u8(last) {
                    return ArgKind::OptionVecU8;
                }
                if name == "Option" {
                    if let Some(inner) = option_inner_type_name(last) {
                        if inner == "String" {
                            return ArgKind::OptionString;
                        }
                        if let Some(decoder) = self.cfg.struct_decoders.get(&inner) {
                            return ArgKind::OptionStructFromJObject {
                                decoder: decoder.clone(),
                                type_name: inner,
                            };
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
                    return ArgKind::StructFromJObject {
                        decoder: decoder.clone(),
                        type_name: name,
                    };
                }
                ArgKind::Unsupported
            }
            _ => ArgKind::Unsupported,
        }
    }
}

/// Field-type classification for `#[prebindgen]` struct fields — narrower
/// than [`ArgKind`] because structs only need a round-trippable primitive /
/// enum representation (no refs, no callbacks, no `Option<...>`).
enum StructFieldKind {
    Bool,
    I64,
    F64,
    Enum(syn::Path),
    Unsupported,
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
    /// `Option<String>` → nullable `JString`.
    OptionString,
    /// Struct type registered via `struct_decoder` → single `JObject` arg
    /// decoded via the registered decoder.
    StructFromJObject {
        decoder: syn::Path,
        type_name: String,
    },
    /// `Option<T>` where `T` is registered via `struct_decoder` → nullable
    /// `JObject`, `None` when the JObject is null.
    OptionStructFromJObject {
        decoder: syn::Path,
        type_name: String,
    },
    /// `impl Fn(T) + Send + Sync + 'static` → `(JObject callback, JObject on_close)`
    /// pair decoded via a callback decoder registered for `T`.
    Callback {
        decoder: syn::Path,
        element_type_name: String,
    },
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

/// Map a Rust snake_case arg name to its Kotlin camelCase form, appending
/// `"Ptr"` for raw-pointer slots (OpaqueRef / consumed KeyExpr).
fn kotlin_param_name(rust_name: &str, is_pointer: bool) -> String {
    let base = snake_to_camel(rust_name);
    if is_pointer {
        format!("{}Ptr", base)
    } else {
        base
    }
}

/// Record `fqn` in `used` if it looks fully-qualified (contains `.`) and
/// return the short name used at the emission site. Bare names are returned
/// as-is and not added to the import set.
fn kotlin_register_fqn(fqn: &str, used: &mut BTreeSet<String>) -> String {
    if fqn.contains('.') {
        used.insert(fqn.to_string());
        fqn.rsplit('.').next().unwrap_or(fqn).to_string()
    } else {
        fqn.to_string()
    }
}
