//! JNI binding generator for functions marked with `#[prebindgen]`.
//!
//! Mirrors the pattern of [`prebindgen::batching::FfiConverter`], but instead
//! of emitting `#[no_mangle] extern "C"` proxy functions, it emits
//! `Java_<class>_<name>ViaJNI` wrappers that decode JNI arguments, call the
//! original Rust function, and wrap the result into a JNI return (or throw a
//! JVM exception on error).
//!
//! # Type registry
//!
//! The converter is fully data-driven: every Rust type that can appear in a
//! `#[prebindgen]` function's signature is described by a [`TypeBinding`]
//! registered up-front via [`Builder::type_binding`]. A binding declares up to
//! four forms:
//!
//! * `consume` — used when the type appears by value as a parameter (`T`);
//! * `borrow`  — used when the type appears as a shared reference (`&T`);
//! * `returns` — used when the type appears in `ZResult<T>` as a return value;
//! * `returns_vec` — used when the type appears as `ZResult<Vec<T>>`.
//!
//! `impl Fn(T) + Send + Sync + 'static` callback parameters reuse the
//! `consume` form on a binding keyed under `"impl Fn(<element>)"`.
//!
//! Each form carries a JNI on-the-wire type (e.g. `jni::sys::jlong`,
//! `jni::objects::JObject`, `*const Foo`), the Kotlin-side declaration, and a
//! decoding/encoding strategy. Built-in bindings for `bool`, `String`,
//! `Vec<u8>`, and `Duration` are pre-registered with sensible defaults; a
//! handful of builder-level convenience methods (`string_decoder`,
//! `byte_array_decoder`, `enum_decoder`, `struct_decoder`, `return_wrapper`,
//! `return_wrapper_vec`) populate or extend bindings without forcing every
//! call site to spell out a full `TypeBinding`.
//!
//! # Pipeline
//!
//! ```ignore
//! use itertools::Itertools;
//! use zenoh_flat::jni_converter::{JniConverter, TypeBinding, JniForm, ArgDecode};
//!
//! let source = prebindgen::Source::new(zenoh_flat::PREBINDGEN_OUT_DIR);
//! let mut converter = JniConverter::builder()
//!     .class_prefix("Java_io_zenoh_jni_JNISession_")
//!     .function_suffix("ViaJNI")
//!     .source_module("zenoh_flat::session")
//!     .owned_object("crate::owned_object::OwnedObject")
//!     .zresult("crate::errors::ZResult")
//!     .throw_exception("crate::throw_exception")
//!     .string_decoder("crate::utils::decode_string")
//!     .byte_array_decoder("crate::utils::decode_byte_array")
//!     .type_binding(
//!         TypeBinding::new("KeyExpr").kotlin("JNIKeyExpr").consume(
//!             JniForm::new(
//!                 "jni::objects::JObject",
//!                 "JObject",
//!                 ArgDecode::env_ref_mut("crate::key_expr::decode_jni_key_expr"),
//!             ),
//!         ),
//!     )
//!     .build();
//! source
//!     .items_all()
//!     .batching(converter.as_closure())
//!     .collect::<prebindgen::collect::Destination>()
//!     .write("zenoh_flat_jni.rs");
//! ```

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;

use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};

use prebindgen::SourceLocation;

// =====================================================================
// TypeBinding: per-type description of JNI representations
// =====================================================================

/// Per-type description of how a Rust type is represented across the JNI
/// boundary. A type may declare up to four forms:
/// `consume` (`T` parameter), `borrow` (`&T` parameter), `returns`
/// (`ZResult<T>` return), and `returns_vec` (`ZResult<Vec<T>>` return).
///
/// Callback parameters (`impl Fn(T) + Send + Sync + 'static`) are described
/// by an ordinary `consume` form on a binding keyed under
/// `"impl Fn(<element>)"` (e.g. `"impl Fn(Sample)"`, `"impl Fn()"`). The
/// classifier synthesizes that key when it sees an `impl Fn(...)` parameter.
#[derive(Clone)]
pub struct TypeBinding {
    name: String,
    /// Kotlin-side type name (FQN preferred — out-of-package import is
    /// auto-derived; bare for same-package). Used as the Kotlin parameter
    /// type when the form's wire JNI type is `JObject` and as the Kotlin
    /// return type. For primitive-mapped forms (`bool`, `Duration`,
    /// `String`, ...) the form's `kotlin_jni_type` is used instead.
    kotlin_type: Option<String>,
    consume: Option<JniForm>,
    borrow: Option<JniForm>,
    returns: Option<ReturnForm>,
    returns_vec: Option<ReturnForm>,
}

/// Strategy for converting a JNI parameter into a Rust value.
#[derive(Clone)]
pub enum ArgDecode {
    /// `let <name> = <path>(&mut env, &<input>)?;`
    EnvRefMut(syn::Path),
    /// `let <name> = <path>(&env, <input>)?;` — used by the legacy
    /// `byte_array_decoder` calling convention.
    EnvByVal(syn::Path),
    /// `let <name> = <path>(<input>)?;` — pure conversion (e.g. enum decoders).
    Pure(syn::Path),
    /// `let <name> = <expr>;` — inline transformation built from the input
    /// ident. Used for trivial conversions like `bool` (`x != 0`) or
    /// `Duration` (`Duration::from_millis(x as u64)`).
    Inline(InlineFn),
    /// `let <name> = <owned_object>::from_raw(<input>);` — borrows the Arc
    /// pointed to by `<input>` via the converter-wide `owned_object` setting.
    /// The argument is passed to the wrapped function as `&<name>`.
    OwnedRef,
    /// Consume an `Arc<T>` raw pointer: reconstructs the Arc, clones the
    /// inner value, and drops the Arc at end of scope.
    ConsumeArc,
}

impl ArgDecode {
    /// `ArgDecode::Pure` from a path string (parsed lazily).
    pub fn pure(path: impl AsRef<str>) -> Self {
        ArgDecode::Pure(syn::parse_str(path.as_ref()).expect("invalid ArgDecode::pure path"))
    }

    /// `ArgDecode::EnvRefMut` from a path string.
    pub fn env_ref_mut(path: impl AsRef<str>) -> Self {
        ArgDecode::EnvRefMut(
            syn::parse_str(path.as_ref()).expect("invalid ArgDecode::env_ref_mut path"),
        )
    }

    /// `ArgDecode::EnvByVal` from a path string.
    pub fn env_by_val(path: impl AsRef<str>) -> Self {
        ArgDecode::EnvByVal(
            syn::parse_str(path.as_ref()).expect("invalid ArgDecode::env_by_val path"),
        )
    }
}

impl ReturnEncode {
    /// `ReturnEncode::Wrapper` from a path string.
    pub fn wrapper(path: impl AsRef<str>) -> Self {
        ReturnEncode::Wrapper(
            syn::parse_str(path.as_ref()).expect("invalid ReturnEncode::wrapper path"),
        )
    }
}

/// Clonable closure that produces a TokenStream from the JNI input ident.
#[derive(Clone)]
pub struct InlineFn(Arc<dyn Fn(&syn::Ident) -> TokenStream + Send + Sync>);

impl InlineFn {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&syn::Ident) -> TokenStream + Send + Sync + 'static,
    {
        InlineFn(Arc::new(f))
    }

    fn call(&self, ident: &syn::Ident) -> TokenStream {
        (self.0)(ident)
    }
}

/// Describes how a JNI parameter for a particular type/form is decoded.
#[derive(Clone)]
pub struct JniForm {
    /// On-the-wire JNI type, e.g. `jni::sys::jlong`, `jni::objects::JObject`,
    /// `*const Session`. Emitted verbatim in the wrapper signature.
    jni_type: syn::Type,
    /// Kotlin-side wire type for this form (`"Long"`, `"Boolean"`, `"Int"`,
    /// `"String"`, `"ByteArray"`, `"JObject"`).
    kotlin_jni_type: String,
    /// True for raw-pointer slots — appends `"Ptr"` to the Kotlin parameter
    /// name (e.g. `sessionPtr: Long`).
    pointer_param: bool,
    decode: ArgDecode,
}

impl JniForm {
    pub fn new(
        jni_type: impl AsRef<str>,
        kotlin_jni_type: impl Into<String>,
        decode: ArgDecode,
    ) -> Self {
        Self {
            jni_type: syn::parse_str(jni_type.as_ref()).expect("invalid JniForm jni_type"),
            kotlin_jni_type: kotlin_jni_type.into(),
            pointer_param: false,
            decode,
        }
    }

    pub fn pointer_param(mut self, p: bool) -> Self {
        self.pointer_param = p;
        self
    }

    /// Whether this form's wire JNI type is a `JObject`-shaped object that
    /// supports `is_null()` (used by the `Option<T>` combinator).
    fn is_jni_object(&self) -> bool {
        matches!(jni_object_kind(&self.jni_type), Some(_))
    }
}

#[derive(Clone)]
pub enum ReturnEncode {
    /// `Ok(<path>(&mut env, __result)?)` — wrapping function returns
    /// `ZResult<jni_type>`.
    Wrapper(syn::Path),
    /// `Ok(Arc::into_raw(Arc::new(__result)))` — opaque Arc-handle return.
    ArcIntoRaw,
}

/// Describes how a Rust return value is encoded into a JNI return.
#[derive(Clone)]
pub struct ReturnForm {
    jni_type: syn::Type,
    kotlin_jni_type: Option<String>,
    encode: ReturnEncode,
    default_expr: syn::Expr,
}

impl ReturnForm {
    pub fn new(
        jni_type: impl AsRef<str>,
        encode: ReturnEncode,
        default_expr: impl AsRef<str>,
    ) -> Self {
        Self {
            jni_type: syn::parse_str(jni_type.as_ref()).expect("invalid ReturnForm jni_type"),
            kotlin_jni_type: None,
            encode,
            default_expr: syn::parse_str(default_expr.as_ref())
                .expect("invalid ReturnForm default_expr"),
        }
    }

    pub fn kotlin(mut self, kotlin: impl Into<String>) -> Self {
        self.kotlin_jni_type = Some(kotlin.into());
        self
    }
}

impl TypeBinding {
    /// Short type name this binding is keyed under (e.g. `"KeyExpr"`).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Construct a binding keyed by `name`. If `name` parses as a Rust type,
    /// it is canonicalized through `quote::ToTokens` so whitespace variations
    /// in user input match the form the classifier produces from AST nodes
    /// (matters for `impl Fn(T) + Send + Sync + 'static`-style names). Falls
    /// back to the literal string if parsing fails.
    pub fn new(name: impl Into<String>) -> Self {
        let raw = name.into();
        let canonical = syn::parse_str::<syn::Type>(&raw)
            .map(|t| t.to_token_stream().to_string())
            .unwrap_or_else(|_| raw);
        Self {
            name: canonical,
            kotlin_type: None,
            consume: None,
            borrow: None,
            returns: None,
            returns_vec: None,
        }
    }

    pub fn kotlin(mut self, fqn: impl Into<String>) -> Self {
        self.kotlin_type = Some(fqn.into());
        self
    }

    pub fn consume(mut self, form: JniForm) -> Self {
        self.consume = Some(form);
        self
    }

    pub fn borrow(mut self, form: JniForm) -> Self {
        self.borrow = Some(form);
        self
    }

    pub fn returns(mut self, form: ReturnForm) -> Self {
        self.returns = Some(form);
        self
    }

    pub fn returns_vec(mut self, form: ReturnForm) -> Self {
        self.returns_vec = Some(form);
        self
    }
}

// =====================================================================
// Builder
// =====================================================================

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
    /// Primary type registry keyed by short type name. Callback registrations
    /// (for `impl Fn(T) + Send + Sync + 'static` parameters) live here too,
    /// as the [`CallbackForm`] slot of the element type's binding.
    types: HashMap<String, TypeBinding>,
    /// Kotlin output config — if `None`, no Kotlin file is emitted.
    kotlin: Option<KotlinConfig>,
}

/// Settings for generating a companion Kotlin file with `external fun`
/// prototypes. Enabled via [`Builder::kotlin_output`].
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
}

impl Default for Builder {
    fn default() -> Self {
        let mut b = Self {
            class_prefix: String::new(),
            function_suffix: String::new(),
            source_module: syn::parse_str("crate").unwrap(),
            struct_source_module: None,
            owned_object: syn::parse_str("OwnedObject").unwrap(),
            zresult: syn::parse_str("ZResult").unwrap(),
            throw_exception: syn::parse_str("throw_exception").unwrap(),
            types: HashMap::new(),
            kotlin: None,
        };
        register_builtins(&mut b.types);
        b
    }
}

/// Pre-register built-in language types (`bool`, `Duration`) plus the
/// scaffolding for `String` / `Vec<u8>` (whose decoders are filled in by
/// [`Builder::string_decoder`] / [`Builder::byte_array_decoder`]).
fn register_builtins(types: &mut HashMap<String, TypeBinding>) {
    // bool — jboolean, inline `x != 0`.
    types.insert(
        "bool".to_string(),
        TypeBinding::new("bool").consume(
            JniForm::new(
                "jni::sys::jboolean",
                "Boolean",
                ArgDecode::Inline(InlineFn::new(|input| quote! { #input != 0 })),
            ),
        ),
    );
    // Duration — jlong, inline `Duration::from_millis(x as u64)`.
    types.insert(
        "Duration".to_string(),
        TypeBinding::new("Duration").consume(
            JniForm::new(
                "jni::sys::jlong",
                "Long",
                ArgDecode::Inline(InlineFn::new(
                    |input| quote! { std::time::Duration::from_millis(#input as u64) },
                )),
            ),
        ),
    );
    // String — JString, decoder filled by string_decoder().
    types.insert(
        "String".to_string(),
        TypeBinding::new("String"),
    );
    // Vec<u8> — keyed under the synthetic name "VecU8" (looked up explicitly
    // by classify_arg when it sees `Vec<u8>`). Decoder filled by
    // byte_array_decoder().
    types.insert(
        "VecU8".to_string(),
        TypeBinding::new("VecU8"),
    );
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

    /// Path of the `OwnedObject` helper used to borrow Arc-pointers in the
    /// `OwnedRef` decode form.
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

    /// Universal entry point: register or replace a [`TypeBinding`] by name.
    /// All sugar methods below delegate to this.
    pub fn type_binding(mut self, binding: TypeBinding) -> Self {
        self.types.insert(binding.name.clone(), binding);
        self
    }

    /// Ingest a reusable [`crate::jni_type_binding::JniTypeBinding`]
    /// collection. All entries are merged into the builder's type registry;
    /// entries in the collection override entries already present in the
    /// builder with the same key.
    pub fn jni_type_binding(
        mut self,
        bindings: crate::jni_type_binding::JniTypeBinding,
    ) -> Self {
        self.types.extend(bindings.types);
        self
    }

    /// Path of the function that decodes a `JString` into `String`. Used by
    /// the built-in `String` binding.
    pub fn string_decoder(mut self, path: impl AsRef<str>) -> Self {
        let p: syn::Path = syn::parse_str(path.as_ref()).expect("invalid string_decoder path");
        let entry = self
            .types
            .get_mut("String")
            .expect("built-in `String` binding missing");
        entry.consume = Some(JniForm::new(
            "jni::objects::JString",
            "String",
            ArgDecode::EnvRefMut(p),
        ));
        self
    }

    /// Path of the function that decodes a `JByteArray` into `Vec<u8>`. Used
    /// by the built-in `Vec<u8>` binding (under the internal name `"VecU8"`).
    pub fn byte_array_decoder(mut self, path: impl AsRef<str>) -> Self {
        let p: syn::Path =
            syn::parse_str(path.as_ref()).expect("invalid byte_array_decoder path");
        let entry = self
            .types
            .get_mut("VecU8")
            .expect("built-in `VecU8` binding missing");
        entry.consume = Some(JniForm::new(
            "jni::objects::JByteArray",
            "ByteArray",
            ArgDecode::EnvByVal(p),
        ));
        self
    }

    /// Enable Kotlin-side prototype generation. `path` is where the `.kt`
    /// file will be written when [`JniConverter::write_kotlin`] is called.
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
    pub fn kotlin_init(mut self, fqn: impl Into<String>) -> Self {
        self.kotlin.get_or_insert_with(KotlinConfig::default).init_load_fqn = Some(fqn.into());
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
        }
    }
}

// =====================================================================
// JniConverter
// =====================================================================

/// Converter that transforms `#[prebindgen]`-marked Rust functions into JNI
/// `Java_*` wrappers.
pub struct JniConverter {
    cfg: Builder,
    pending: VecDeque<(syn::Item, SourceLocation)>,
    /// `true` once the source iterator has been drained and sorted (structs
    /// before functions) so that function-arg classification can see every
    /// auto-registered struct binding.
    buffered: bool,
    /// Accumulated Kotlin `external fun ...` blocks, one per wrapped function.
    kotlin_funs: Vec<String>,
    /// Accumulated Kotlin `data class ...` blocks, one per `#[prebindgen]`
    /// struct seen in the source stream.
    kotlin_data_classes: Vec<String>,
    /// Set of Kotlin FQNs referenced by emitted externals.
    kotlin_used_fqns: BTreeSet<String>,
}

impl JniConverter {
    pub fn builder() -> Builder {
        Builder::default()
    }

    /// Drain `iter` on the first call, sort so `#[prebindgen]` struct items
    /// are processed before functions (so function-arg classification can see
    /// every auto-registered struct binding), then return converted items
    /// one at a time from the buffer.
    pub fn call<I>(&mut self, iter: &mut I) -> Option<(syn::Item, SourceLocation)>
    where
        I: Iterator<Item = (syn::Item, SourceLocation)>,
    {
        if !self.buffered {
            self.buffered = true;
            let mut all: Vec<(syn::Item, SourceLocation)> = iter.by_ref().collect();
            all.sort_by_key(|(it, _)| !matches!(it, syn::Item::Struct(_)));
            for (item, loc) in all {
                let converted = self.convert(item, &loc);
                self.pending.push_back((converted, loc));
            }
        }
        self.pending.pop_front()
    }

    /// Closure suitable for `itertools::batching`. Consumes `self`.
    pub fn into_closure<I>(
        mut self,
    ) -> impl FnMut(&mut I) -> Option<(syn::Item, SourceLocation)>
    where
        I: Iterator<Item = (syn::Item, SourceLocation)>,
    {
        move |iter| self.call(iter)
    }

    /// Borrowing closure suitable for `itertools::batching`. Unlike
    /// [`JniConverter::into_closure`], this does not consume `self`, so
    /// [`JniConverter::write_kotlin`] can be called after the pipeline.
    pub fn as_closure<'a, I>(
        &'a mut self,
    ) -> impl FnMut(&mut I) -> Option<(syn::Item, SourceLocation)> + 'a
    where
        I: Iterator<Item = (syn::Item, SourceLocation)>,
    {
        move |iter| self.call(iter)
    }

    /// Write the accumulated Kotlin `external fun ...` prototypes to the
    /// configured output path. No-op when Kotlin output was not enabled.
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

            let kind = self.classify_arg(ty, name);
            self.emit_arg(
                kind,
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
                } else if let Some(form) = self.lookup_return_form(&inner) {
                    if kt_enabled {
                        let kt = form
                            .kotlin_jni_type
                            .clone()
                            .expect("return form Kotlin type not configured");
                        let short = kotlin_register_fqn(&kt, &mut local_kotlin_fqns);
                        kotlin_ret = Some(short);
                    }
                    let jni_type = &form.jni_type;
                    let default_expr = &form.default_expr;
                    match &form.encode {
                        ReturnEncode::Wrapper(wrap_fn) => (
                            quote! { #jni_type },
                            quote! { #wrap_fn(&mut env, __result) },
                            quote! { #default_expr },
                            quote! { #zresult<#jni_type> },
                        ),
                        ReturnEncode::ArcIntoRaw => (
                            quote! { #jni_type },
                            quote! {
                                Ok(std::sync::Arc::into_raw(std::sync::Arc::new(__result)))
                            },
                            quote! { #default_expr },
                            quote! { #zresult<#jni_type> },
                        ),
                    }
                } else {
                    // Fallback: treat as opaque Arc-handle, return `*const T`.
                    if kt_enabled {
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

    /// Emit a JNI decoder for a `#[prebindgen]` struct and a matching Kotlin
    /// `data class`, then auto-register a `TypeBinding` so the struct can
    /// appear by value in a wrapped function's signature.
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

        // Auto-register a TypeBinding for this struct.
        let decoder_path: syn::Path = syn::parse_str(&format!("decode_{struct_name}"))
            .expect("generated decoder ident must parse as path");
        let mut binding = TypeBinding::new(struct_name.clone());
        binding.kotlin_type = Some(struct_name.clone()); // bare — same package
        binding.consume = Some(JniForm::new(
            "jni::objects::JObject",
            "JObject",
            ArgDecode::EnvRefMut(decoder_path),
        ));
        self.cfg.types.insert(struct_name.clone(), binding);

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
                // Enum decoders are stored as a `Pure` ArgDecode on the
                // type's binding's `consume` form.
                if let Some(binding) = self.cfg.types.get(&name) {
                    if let Some(form) = binding.consume.as_ref() {
                        if let ArgDecode::Pure(p) = &form.decode {
                            return StructFieldKind::Enum(p.clone());
                        }
                    }
                }
                StructFieldKind::Unsupported
            }
        }
    }

    fn lookup_return_form(&self, ty: &syn::Type) -> Option<&ReturnForm> {
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
            return self
                .cfg
                .types
                .get(&elem_name)
                .and_then(|b| b.returns_vec.as_ref());
        }
        self.cfg.types.get(&name).and_then(|b| b.returns.as_ref())
    }

    /// Classify a function-arg type into one of the uniform variants below.
    fn classify_arg(&self, ty: &syn::Type, _name: &syn::Ident) -> ArgKind {
        match ty {
            syn::Type::Reference(r) if r.mutability.is_none() => {
                let elem = &*r.elem;
                let last = type_last_segment(elem).unwrap_or_default();
                if let Some(binding) = self.cfg.types.get(&last) {
                    if let Some(form) = binding.borrow.as_ref() {
                        return ArgKind::Borrow {
                            form: form.clone(),
                            kotlin_override: binding.kotlin_type.clone(),
                        };
                    }
                }
                // Fallback: treat any unbound `&T` as an `OwnedRef` against a
                // raw `*const T` pointer, decoded via the converter-wide
                // `owned_object`. This matches the legacy `OpaqueRef` path.
                let ptr_ty: syn::Type = syn::parse2(quote! { *const #elem })
                    .expect("opaque pointer type must parse");
                let opaque_form = JniForm {
                    jni_type: ptr_ty,
                    kotlin_jni_type: "Long".to_string(),
                    pointer_param: true,
                    decode: ArgDecode::OwnedRef,
                };
                ArgKind::Borrow {
                    form: opaque_form,
                    kotlin_override: None,
                }
            }
            syn::Type::ImplTrait(_) => {
                let key = ty.to_token_stream().to_string();
                if let Some(binding) = self.cfg.types.get(&key) {
                    if let Some(form) = binding.consume.as_ref() {
                        return ArgKind::Consume {
                            form: form.clone(),
                            kotlin_override: binding.kotlin_type.clone(),
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

                if name == "Option" {
                    let inner_seg = last;
                    if is_option_of_vec_u8(inner_seg) {
                        if let Some(binding) = self.cfg.types.get("VecU8") {
                            if let Some(form) = binding.consume.as_ref() {
                                return ArgKind::OptionConsume {
                                    form: form.clone(),
                                    kotlin_override: None,
                                };
                            }
                        }
                        return ArgKind::Unsupported;
                    }
                    if let Some(inner) = option_inner_type_name(inner_seg) {
                        if let Some(binding) = self.cfg.types.get(&inner) {
                            if let Some(form) = binding.consume.as_ref() {
                                return ArgKind::OptionConsume {
                                    form: form.clone(),
                                    kotlin_override: binding.kotlin_type.clone(),
                                };
                            }
                        }
                    }
                    return ArgKind::Unsupported;
                }

                if name == "Vec" && is_vec_of_u8(last) {
                    if let Some(binding) = self.cfg.types.get("VecU8") {
                        if let Some(form) = binding.consume.as_ref() {
                            return ArgKind::Consume {
                                form: form.clone(),
                                kotlin_override: None,
                            };
                        }
                    }
                    return ArgKind::Unsupported;
                }

                if let Some(binding) = self.cfg.types.get(&name) {
                    if let Some(form) = binding.consume.as_ref() {
                        return ArgKind::Consume {
                            form: form.clone(),
                            kotlin_override: binding.kotlin_type.clone(),
                        };
                    }
                }

                ArgKind::Unsupported
            }
            _ => ArgKind::Unsupported,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_arg(
        &self,
        kind: ArgKind,
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
        match kind {
            ArgKind::Consume {
                form,
                kotlin_override,
            } => {
                self.emit_consume_or_borrow(
                    /* borrow */ false,
                    form,
                    kotlin_override,
                    name,
                    prelude,
                    jni_params,
                    call_args,
                    kotlin_params,
                    local_kotlin_fqns,
                    kt_enabled,
                );
            }
            ArgKind::Borrow {
                form,
                kotlin_override,
            } => {
                self.emit_consume_or_borrow(
                    /* borrow */ true,
                    form,
                    kotlin_override,
                    name,
                    prelude,
                    jni_params,
                    call_args,
                    kotlin_params,
                    local_kotlin_fqns,
                    kt_enabled,
                );
            }
            ArgKind::OptionConsume {
                form,
                kotlin_override,
            } => {
                if !form.is_jni_object() {
                    panic!(
                        "Option<{}> requires a JNI-object form for `{}`",
                        ty.to_token_stream(),
                        name
                    );
                }
                let jt = &form.jni_type;
                jni_params.push(quote! { #name: #jt });
                let inner = self.decode_expr(&form.decode, name);
                prelude.push(quote! {
                    let #name = if !#name.is_null() {
                        Some(#inner)
                    } else {
                        None
                    };
                });
                call_args.push(quote! { #name });
                if kt_enabled {
                    let kt_decl = self.kotlin_arg_type(&form, kotlin_override.as_deref());
                    let short = kotlin_register_fqn(&kt_decl, local_kotlin_fqns);
                    kotlin_params.push(format!(
                        "{}: {}?",
                        kotlin_param_name(&name.to_string(), form.pointer_param),
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

    #[allow(clippy::too_many_arguments)]
    fn emit_consume_or_borrow(
        &self,
        borrow: bool,
        form: JniForm,
        kotlin_override: Option<String>,
        name: &syn::Ident,
        prelude: &mut Vec<TokenStream>,
        jni_params: &mut Vec<TokenStream>,
        call_args: &mut Vec<TokenStream>,
        kotlin_params: &mut Vec<String>,
        local_kotlin_fqns: &mut BTreeSet<String>,
        kt_enabled: bool,
    ) {
        let jt = &form.jni_type;
        let pat = if matches!(form.decode, ArgDecode::OwnedRef | ArgDecode::ConsumeArc) {
            // Raw-pointer slots get a `_ptr` ident in the JNI signature.
            format_ident!("{}_ptr", name)
        } else {
            name.clone()
        };
        jni_params.push(quote! { #pat: #jt });

        match &form.decode {
            ArgDecode::EnvRefMut(path) => {
                prelude.push(quote! { let #name = #path(&mut env, &#pat)?; });
            }
            ArgDecode::EnvByVal(path) => {
                prelude.push(quote! { let #name = #path(&env, #pat)?; });
            }
            ArgDecode::Pure(path) => {
                prelude.push(quote! { let #name = #path(#pat)?; });
            }
            ArgDecode::Inline(f) => {
                let expr = f.call(&pat);
                prelude.push(quote! { let #name = #expr; });
            }
            ArgDecode::OwnedRef => {
                let owned = &self.cfg.owned_object;
                prelude.push(quote! { let #name = #owned::from_raw(#pat); });
            }
            ArgDecode::ConsumeArc => {
                let arc_ident = format_ident!("__{}_arc", name);
                prelude.push(quote! {
                    let #arc_ident = std::sync::Arc::from_raw(#pat);
                    let #name = (*#arc_ident).clone();
                });
            }
        }

        // The wrapped function call site: by-value vs by-reference.
        if borrow && matches!(form.decode, ArgDecode::OwnedRef) {
            // OwnedRef pattern: pass `&name` to match historical OpaqueRef behavior.
            call_args.push(quote! { &#name });
        } else {
            call_args.push(quote! { #name });
        }

        if kt_enabled {
            let kt_decl = self.kotlin_arg_type(&form, kotlin_override.as_deref());
            let short = kotlin_register_fqn(&kt_decl, local_kotlin_fqns);
            kotlin_params.push(format!(
                "{}: {}",
                kotlin_param_name(&name.to_string(), form.pointer_param),
                short
            ));
        }
    }

    fn decode_expr(&self, decode: &ArgDecode, input: &syn::Ident) -> TokenStream {
        match decode {
            ArgDecode::EnvRefMut(path) => quote! { #path(&mut env, &#input)? },
            ArgDecode::EnvByVal(path) => quote! { #path(&env, #input)? },
            ArgDecode::Pure(path) => quote! { #path(#input)? },
            ArgDecode::Inline(f) => f.call(input),
            ArgDecode::OwnedRef => {
                let owned = &self.cfg.owned_object;
                quote! { #owned::from_raw(#input) }
            }
            ArgDecode::ConsumeArc => {
                quote! { (*std::sync::Arc::from_raw(#input)).clone() }
            }
        }
    }

    /// Resolve the Kotlin parameter type for a JniForm. For object-typed
    /// JNI wires (`JObject`) we use the binding's `kotlin_type` FQN; for
    /// primitive wires (jboolean/jlong/jint/JString/JByteArray/raw ptrs) we
    /// use the form's `kotlin_jni_type`.
    fn kotlin_arg_type(&self, form: &JniForm, kotlin_override: Option<&str>) -> String {
        match jni_object_kind(&form.jni_type) {
            Some(JniObjectKind::JObject) => kotlin_override
                .map(str::to_string)
                .unwrap_or_else(|| form.kotlin_jni_type.clone()),
            _ => form.kotlin_jni_type.clone(),
        }
    }
}

// =====================================================================
// ArgKind — the slim, uniform classification
// =====================================================================

enum ArgKind {
    Consume {
        form: JniForm,
        kotlin_override: Option<String>,
    },
    Borrow {
        form: JniForm,
        kotlin_override: Option<String>,
    },
    OptionConsume {
        form: JniForm,
        kotlin_override: Option<String>,
    },
    Unsupported,
}

/// Field-type classification for `#[prebindgen]` struct fields — narrower
/// than [`ArgKind`] because structs only need a round-trippable primitive /
/// enum representation.
enum StructFieldKind {
    Bool,
    I64,
    F64,
    Enum(syn::Path),
    Unsupported,
}

#[derive(Clone, Copy)]
enum JniObjectKind {
    JObject,
    JString,
    JByteArray,
}

/// Recognize JNI object-shaped wire types. Used by the `Option<T>` combinator
/// (which needs `is_null()`) and by the Kotlin parameter-type derivation.
fn jni_object_kind(ty: &syn::Type) -> Option<JniObjectKind> {
    let syn::Type::Path(tp) = ty else { return None };
    let last = tp.path.segments.last()?;
    match last.ident.to_string().as_str() {
        "JObject" => Some(JniObjectKind::JObject),
        "JString" => Some(JniObjectKind::JString),
        "JByteArray" => Some(JniObjectKind::JByteArray),
        _ => None,
    }
}

fn type_last_segment(ty: &syn::Type) -> Option<String> {
    let syn::Type::Path(tp) = ty else { return None };
    tp.path.segments.last().map(|s| s.ident.to_string())
}

/// Last-segment name of the single generic argument of an `Option<...>`.
fn option_inner_type_name(seg: &syn::PathSegment) -> Option<String> {
    let syn::PathArguments::AngleBracketed(args) = &seg.arguments else {
        return None;
    };
    let syn::GenericArgument::Type(inner) = args.args.first()? else {
        return None;
    };
    type_last_segment(inner)
}

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
