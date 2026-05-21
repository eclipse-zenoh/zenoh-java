//! JNI implementation of [`PrebindgenExt`].
//!
//! Provides the universal JNI patterns:
//! * **Wrapper signatures**: input converter is
//!   `fn(env: &mut JNIEnv, v: <wire>) -> ZResult<<rust>>`; output converter
//!   is `fn(env: &mut JNIEnv, v: &<rust>) -> ZResult<<wire>>`.
//! * **`on_function`**: emits a JNI `extern "C"` wrapper that delegates each
//!   parameter conversion to the auto-generated `<rust>_to_<wire>_<hash>`
//!   converter, calls the original `#[prebindgen]` fn, and routes errors
//!   through the generated `throw_<RustShortName>` free function emitted
//!   alongside the registered `kotlin_exception_class` entries.
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

use std::collections::HashMap;
use std::sync::Arc;

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, ToTokens};

use crate::core::niches::Niches;
use crate::core::prebindgen_ext::{ConverterImpl, IntoSource, IntoSourceMode, PrebindgenExt};
use crate::core::registry::{extract_fn_trait_args, Registry, TypeKey};
use crate::jni::wire_access::jni_field_access;
use crate::util::snake_to_camel;

// ──────────────────────────────────────────────────────────────────────
// Language metadata (PrebindgenExt::Metadata for JniExt)
// ──────────────────────────────────────────────────────────────────────

/// Per-converter language-specific extras carried by every
/// [`ConverterImpl`] this back-end produces. Filled by the same handler
/// that builds the wire/body, propagated by the resolver into
/// [`crate::core::registry::TypeEntry::metadata`], and read directly by
/// the Kotlin emitter — so cross-language facts flow through the
/// existing wrapper machinery rather than a parallel side channel.
#[derive(Clone, Debug, Default)]
pub struct KotlinMeta {
    /// Value-context Kotlin type name. `Some("Long")` for opaque
    /// handles (jlong wire mention), `Some("io.zenoh.jni.JNIEncoding")`
    /// for user-declared decoder types whose wire isn't primitive,
    /// `Some("List<ByteArray>")` when a wrapper composes a primitive
    /// inner. `None` only for entries that must not appear in any
    /// Kotlin signature — the emitter treats that as a hard error.
    pub kotlin_name: Option<String>,
    /// Kotlin fully-qualified exception class this converter can raise
    /// when used as a function's return-type output converter. Populated
    /// by [`JniExt::output_throws`]; the Kotlin emitter uses this for
    /// `@Throws` annotations on the corresponding wrappers. `None` means
    /// "non-throwing converter" (no `@Throws` emitted).
    pub throws: Option<String>,
    /// Rust path of the generated `throw_<RustShortName>` free function
    /// the framework invokes as `<throws_action>(&mut env, &err)` for
    /// wrapper-internal failures (e.g. input-decode `?` propagation) that
    /// surface above this converter. Populated alongside
    /// [`Self::throws`] by [`JniExt::output_throws`]; `None` when no
    /// throwing behavior is configured for this converter. Replaces the
    /// earlier `throw_exception!` macro path with a direct function call
    /// emitted by [`JniExt::write_exceptions_rust`].
    pub throws_action: Option<syn::Path>,
    /// For wrapper converters whose Kotlin projection is the *inner*
    /// type's projection (e.g. `ZResult<Publisher>` → `Publisher`),
    /// this carries the inner Rust type's canonical key so downstream
    /// emitters (typed-handle constructor lookup in
    /// [`crate::jni::jni_kotlin_ext::classify_return`]) can find the
    /// wrapped value's identity without baking in any specific shape
    /// (no `peel_zresult` / `peel_result`-style framework hardcoding).
    /// Populated by [`JniExt::output_throws`] with `args[0]`'s canonical
    /// key for arity-1 wrappers, and inherited by the built-in
    /// `Option<_>` / `Vec<_>` / `&_` rank-1 handlers from their inner
    /// type's metadata. `None` for plain values and arity-0 converters.
    pub value_rust_key: Option<String>,
}

impl KotlinMeta {
    pub fn from_name(name: impl Into<String>) -> Self {
        Self {
            kotlin_name: Some(name.into()),
            throws: None,
            throws_action: None,
            value_rust_key: None,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────
// Structured type-conversion configuration
// ──────────────────────────────────────────────────────────────────────

/// Specification for a single direction of a custom converter — the wire
/// type the host language sees plus the converter body expression.
#[derive(Clone)]
pub(crate) struct ConverterSpec {
    pub wire: syn::Type,
    pub body: syn::Expr,
    pub niches: Niches,
}

/// Per-exception-class configuration (driven by
/// [`JniExt::kotlin_exception_class`]).
///
/// One entry per Rust error type the binding surfaces to the JVM as a
/// Java exception. Declaration order matters: `exceptions[0]` is the
/// *primary* — its `From<String>` impl is the universal converter-failure
/// path and its Kotlin FQN is used for `NativeHandle`'s closed-handle
/// exception. The throw function emitted into the generated file
/// (`throw_<rust_short>`) does the `find_class`/`throw_new` dance and
/// is referenced by the `output_throws`-driven wrapper code through
/// [`KotlinMeta::throws_action`].
#[derive(Clone)]
pub(crate) struct ExceptionConfig {
    /// Absolute Rust path of the error type (e.g.
    /// `zenoh_flat::errors::ZError`). Used both to splice the
    /// `pub(crate) type __JniErr = ...` alias and as the function-
    /// argument type of the generated `throw_<short>`.
    pub rust_path: syn::Path,
    /// Last path segment of `rust_path` (e.g. `"ZError"`). Used to
    /// derive the `throw_<short>` function name and to provide the
    /// default Kotlin class name when no `.kotlin_name(...)` override
    /// is supplied.
    pub rust_short: String,
    /// Kotlin fully-qualified exception class name (e.g.
    /// `"io.zenoh.jni.ZError"`). Defaults to `<package>.<rust_short>`;
    /// overridden by a chained `.kotlin_name(...)`. Used for the Kotlin
    /// class file path, `@Throws` annotations, and the JNI
    /// `find_class("io/zenoh/jni/ZError")` literal inside the generated
    /// `throw_<short>` body.
    pub kotlin_fqn: String,
    /// Identifier of the generated `throw_<short>` function. Stable
    /// across `.kotlin_name(...)` overrides (those only change the
    /// Kotlin class, not the Rust function name).
    pub throw_fn_name: syn::Ident,
    /// Result-shape patterns owned by this exception (set by chained
    /// `.output_throws(pattern, builder)` calls). Each pattern's
    /// resolved converter raises this exception on `Err`.
    pub output_throws: Vec<ExceptionOutputThrows>,
}

/// One `.output_throws(pattern, builder)` registration scoped to a
/// particular [`ExceptionConfig`]. Holds only the pattern's canonical
/// key — the per-rank wrapper closure is stored in the regular
/// [`JniExt::output_wrappers`] tables, looked up by the same key.
#[derive(Clone)]
pub(crate) struct ExceptionOutputThrows {
    pub pattern_key: TypeKey,
}

/// Per-opaque-handle configuration (driven by `JniExt::kotlin_class`).
///
/// The typed-handle Kotlin FQN (e.g. `"io.zenoh.jni.JNISession"`) lives
/// in the surrounding [`TypeConfig::kotlin_name`] slot — FQN-consumers
/// (typed-handle class emission, `instanceof` dispatch,
/// return-value constructor wrap) read it from there. The
/// value-context Kotlin name for the same type (`"Long"`) is produced
/// independently by the rank-0 opaque handler in [`KotlinMeta`], so
/// the two roles don't collide despite sharing the `TypeConfig`.
#[derive(Clone, Default)]
pub(crate) struct OpaqueConfig {
    /// When `false` (default), the unified Kotlin emitter writes a
    /// typed-handle `.kt` file for this opaque type. Set to `true` by
    /// [`JniExt::suppress_kotlin_code`] to indicate the Kotlin file is
    /// hand-maintained — only the Rust-side converter and `instanceof`
    /// dispatch wire up.
    pub suppress_kotlin_code: bool,
    /// `#[prebindgen]` fn idents promoted to instance methods on the
    /// matching Kotlin typed-handle class. Filled by repeated
    /// [`JniExt::method`] calls.
    pub methods: Vec<String>,
}

/// All configuration the structured builder accumulates for one
/// canonical Rust type key. Every field is `None` by default;
/// builder methods populate the ones they care about.
#[derive(Clone, Default)]
pub(crate) struct TypeConfig {
    /// Short Kotlin name or FQN. Required for any type emitted in
    /// Kotlin (`Sample` → `"io.zenoh.jni.Sample"`,
    /// `Vec<u8>` → `"ByteArray"`).
    pub kotlin_name: Option<String>,
    /// If `Some`, this is an opaque-handle type — gets jlong wire,
    /// `Box::into_raw`/`Box::from_raw` conventions, instanceof
    /// dispatch, and Kotlin typed-handle class emission.
    pub opaque: Option<OpaqueConfig>,
    /// Custom input converter override (rank-0). Wins over JniExt's
    /// primitive defaults.
    pub input: Option<ConverterSpec>,
    /// Custom output converter override (rank-0).
    pub output: Option<ConverterSpec>,
    /// Kotlin FQN override for `impl Fn(...)` keys (replaces the
    /// auto-derived `JNI<Stem>Callback` name).
    pub callback_kotlin_fqn: Option<String>,
}

/// Boxed closure that builds the wire/body for a wrapper converter when
/// applied to the wildcard substitutions. Returns `None` if the inner
/// converters the builder depends on aren't yet resolved (the resolver
/// retries on the next phase). Receives `&Registry<KotlinMeta>` so the
/// closure can look up inner-type entries (`registry.output_entry(t)`).
pub(crate) type WrapperFn = Arc<
    dyn Fn(&[syn::Type], &Registry<KotlinMeta>) -> Option<(syn::Type, syn::Expr)>
        + Send
        + Sync,
>;

/// Trait selecting the arity-appropriate impl of
/// [`JniExt::input_wrapper`] / [`JniExt::output_wrapper`] /
/// [`JniExt::output_throws`]. The phantom type parameter discriminates
/// closures of arity 0..3 so a single public method name accepts any of
/// them. Closures take the wildcard substitutions plus the registry, and
/// return `Some((wire, body))` or `None` (defer to a later resolver phase).
pub trait WrapperBuilder<Arity>: Send + Sync + 'static {
    fn into_wrapper_fn(self) -> WrapperFn;
    fn rank() -> usize;
}

/// Arity-discriminating marker types. `Arity0` is for non-wildcard
/// patterns (e.g. `"i32"`); `Arity1`/`2`/`3` carry that many `_` slots.
pub struct Arity0;
pub struct Arity1;
pub struct Arity2;
pub struct Arity3;

impl<F> WrapperBuilder<Arity0> for F
where
    F: Fn(&Registry<KotlinMeta>) -> Option<(syn::Type, syn::Expr)>
        + Send
        + Sync
        + 'static,
{
    fn into_wrapper_fn(self) -> WrapperFn {
        Arc::new(move |_args: &[syn::Type], reg: &Registry<KotlinMeta>| self(reg))
    }
    fn rank() -> usize { 0 }
}

impl<F> WrapperBuilder<Arity1> for F
where
    F: Fn(&syn::Type, &Registry<KotlinMeta>) -> Option<(syn::Type, syn::Expr)>
        + Send
        + Sync
        + 'static,
{
    fn into_wrapper_fn(self) -> WrapperFn {
        Arc::new(move |args: &[syn::Type], reg: &Registry<KotlinMeta>| {
            self(&args[0], reg)
        })
    }
    fn rank() -> usize { 1 }
}

impl<F> WrapperBuilder<Arity2> for F
where
    F: Fn(&syn::Type, &syn::Type, &Registry<KotlinMeta>) -> Option<(syn::Type, syn::Expr)>
        + Send
        + Sync
        + 'static,
{
    fn into_wrapper_fn(self) -> WrapperFn {
        Arc::new(move |args: &[syn::Type], reg: &Registry<KotlinMeta>| {
            self(&args[0], &args[1], reg)
        })
    }
    fn rank() -> usize { 2 }
}

impl<F> WrapperBuilder<Arity3> for F
where
    F: Fn(&syn::Type, &syn::Type, &syn::Type, &Registry<KotlinMeta>)
            -> Option<(syn::Type, syn::Expr)>
        + Send
        + Sync
        + 'static,
{
    fn into_wrapper_fn(self) -> WrapperFn {
        Arc::new(move |args: &[syn::Type], reg: &Registry<KotlinMeta>| {
            self(&args[0], &args[1], &args[2], reg)
        })
    }
    fn rank() -> usize { 3 }
}

/// JNI back-end. Configure paths in the Rust crate (zresult, throw macro,
/// source module the original fns live in) and the JNI/Kotlin classpath
/// (java class prefix, callback Kotlin package + output dir).
#[derive(Clone)]
pub struct JniExt {
    /// Module path the original `#[prebindgen]` fns live under (e.g.
    /// the host crate of `#[prebindgen]` items). The wrapper body calls
    /// `<source_module>::<fn>(args)`.
    pub source_module: syn::Path,
    /// Registered exception classes in declaration order. The first entry
    /// (`exceptions[0]`) is the *primary* — its `From<String>` impl is the
    /// universal converter-failure path (emitted as the `__JniErr` alias
    /// by [`Self::prerequisites`]) and its Kotlin FQN is used for
    /// `NativeHandle`'s closed-handle exception. Populated by repeated
    /// [`Self::kotlin_exception_class`] calls; consumed by:
    /// [`Self::prerequisites`] (primary error type → `__JniErr`),
    /// [`Self::write_exceptions_rust`] (one `throw_<short>` per entry),
    /// [`Self::write_native_handle`] (primary FQN), and
    /// [`Self::lookup_output_wrapper`] (per-pattern FQN + throw fn).
    pub(crate) exceptions: Vec<ExceptionConfig>,
    /// Single source of truth for the JVM/Kotlin namespace this binding
    /// targets, dot-separated (e.g. `io.zenoh.jni`). Empty = no prefix.
    /// Drives every derived form: slash-separated for `FindClass`,
    /// `_`-mangled for JNI extern idents, and dot-separated for Kotlin
    /// `package` declarations.
    pub package: String,
    /// JVM class on which the JNI `external fun`s live. Combined with
    /// [`Self::package`] to produce the JNI symbol prefix
    /// `Java_<package_underscores>_<jni_native_class>_<fn><suffix>`.
    pub jni_native_class: String,
    /// Sub-package leaf appended to [`Self::package`] for the auto-emitted
    /// callback fun-interface files. Combined as
    /// `<package>.<callback_subpackage>`; empty = same package as
    /// [`Self::package`].
    pub callback_subpackage: String,
    /// Prefix applied to the auto-derived callback class name. Combined
    /// with the per-callback `stem` and [`Self::callback_name_postfix`]:
    /// `<prefix><stem><postfix>`.
    pub callback_name_prefix: String,
    /// Postfix applied to the auto-derived callback class name.
    pub callback_name_postfix: String,
    /// Derived: `package.replace('.', '/')`. Read by
    /// [`struct_output_body`] when building `FindClass` strings.
    pub(crate) java_class_prefix: String,
    /// Derived: `"Java_" + package.replace('.', '_') + "_" + jni_native_class`.
    /// Read by [`Self::on_function`] when mangling fn idents.
    pub(crate) jni_class_path: String,
    /// Suffix appended to the wrapped fn name (e.g. `ViaJNI`).
    pub jni_method_suffix: String,
    /// Derived: `package + "." + callback_subpackage` (or just `package`
    /// when the subpackage is empty). Also drives the on-disk subdirectory
    /// under the `kotlin_root` passed to [`Self::write_kotlin`]
    /// (`a.b.c` → `a/b/c/`).
    pub(crate) kotlin_callback_package: String,
    /// Derived `<rust-type-canonical-string> → <kotlin FQN>` view —
    /// populated alongside [`Self::types`] by the structured builders
    /// ([`Self::kotlin_class`], [`Self::kotlin_value_type`],
    /// [`Self::callback_kotlin_name`]). Internal readers
    /// (`emit_into_dispatcher`, callback FQN merging) consume this
    /// flat list directly; the structured `types` map is the source
    /// of truth.
    pub(crate) kotlin_type_fqns: Vec<(String, String)>,

    /// Structured per-type configuration keyed by canonical Rust type.
    /// One entry per `Rust type ↔ JNI/Kotlin` rule; populated by the
    /// structured builders (`kotlin_class`, `input_decoder`,
    /// `output_encoder`, `jint_enum`, `callback_input`,
    /// `callback_kotlin_name`, `kotlin_value_type`). Consulted first by
    /// the [`PrebindgenExt`] rank-0 methods and by all Kotlin emitters.
    pub(crate) types: HashMap<TypeKey, TypeConfig>,

    /// `impl Into<target> + Send + 'static` source arms per target type.
    pub(crate) into_sources_map: HashMap<TypeKey, Vec<IntoSource>>,

    /// Per-rank input wrappers — index `n` holds rank-`n` wrappers keyed by
    /// the pattern's `TypeKey`. Rank 0 is non-wildcard (e.g. `"i32"`); ranks
    /// 1..3 carry that many `_` slots (e.g. `"Vec < _ >"`).
    pub(crate) input_wrappers: [HashMap<TypeKey, WrapperFn>; 4],

    /// Per-rank output wrappers. Same shape as [`Self::input_wrappers`].
    pub(crate) output_wrappers: [HashMap<TypeKey, WrapperFn>; 4],

    /// Tracks the last [`Self::kotlin_class`] key registered so
    /// [`Self::method`] / [`Self::suppress_kotlin_code`] know which
    /// entry to extend. Cleared after each unrelated builder call.
    last_opaque_key: Option<TypeKey>,

    /// Tracks the last decoder/encoder key registered so
    /// [`Self::with_kotlin_name`] knows which entry to stamp. Cleared
    /// after each unrelated builder call.
    last_meta_key: Option<TypeKey>,

    /// Tracks the last [`Self::kotlin_exception_class`] registration so
    /// chained [`Self::kotlin_name`] / [`Self::output_throws`] calls know
    /// which entry of [`Self::exceptions`] to mutate. Stored as an index
    /// rather than a key because exception entries are identified by
    /// position (declaration order = primary-first). Cleared after each
    /// unrelated builder call.
    last_exception_idx: Option<usize>,
}

impl JniExt {
    /// Convenience constructor with sensible defaults; the paths still need
    /// to be set explicitly via the field-mutation builder methods.
    pub fn new() -> Self {
        Self {
            source_module: syn::parse_str("crate").unwrap(),
            // No exceptions until the binding calls
            // `.kotlin_exception_class(...)` at least once.
            // `prerequisites()` panics with a clear message if the
            // primary entry is still missing at write time.
            exceptions: Vec::new(),
            package: String::new(),
            jni_native_class: "JNINative".to_string(),
            callback_subpackage: "callbacks".to_string(),
            callback_name_prefix: "JNI".to_string(),
            callback_name_postfix: "Callback".to_string(),
            java_class_prefix: String::new(),
            jni_class_path: "Java_JNINative".to_string(),
            jni_method_suffix: String::new(),
            kotlin_callback_package: "callbacks".to_string(),
            kotlin_type_fqns: Vec::new(),
            types: HashMap::new(),
            into_sources_map: HashMap::new(),
            input_wrappers: [
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
            ],
            output_wrappers: [
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
            ],
            last_opaque_key: None,
            last_meta_key: None,
            last_exception_idx: None,
        }
    }
    pub fn source_module(mut self, p: impl AsRef<str>) -> Self {
        self.source_module = syn::parse_str(p.as_ref()).expect("invalid source_module path");
        self
    }

    /// Declare a Rust error type that crosses the JNI boundary as a Java
    /// exception. The first call registers the *primary* exception: it
    /// must impl `From<String>` (the universal converter-failure path —
    /// threaded as `__JniErr` through every generated `Result<_, E>`
    /// signature) and its Kotlin FQN is used for `NativeHandle`'s
    /// closed-handle exception. Subsequent calls register additional
    /// exception types; each one gets its own generated Kotlin class and
    /// its own `throw_<RustShortName>` free function.
    ///
    /// `rust_path` is the absolute Rust path of the error type (e.g.
    /// `"zenoh_flat::errors::ZError"`); the type must impl `Display`.
    /// The default Kotlin class name is `<package>.<rust_short>`;
    /// chain [`Self::kotlin_name`] to override.
    ///
    /// Chain [`Self::output_throws`] immediately after to associate
    /// result-shape patterns (`"ZResult<_>"`, `"()"`) with this
    /// exception — the matching converter's `Err` branch invokes this
    /// exception's `throw_<short>` function.
    pub fn kotlin_exception_class(mut self, rust_path: impl AsRef<str>) -> Self {
        let path_str = rust_path.as_ref();
        let path: syn::Path = syn::parse_str(path_str)
            .unwrap_or_else(|e| panic!("kotlin_exception_class: invalid rust path `{}`: {}", path_str, e));
        let short = path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_else(|| panic!(
                "kotlin_exception_class: rust path `{}` has no segments", path_str
            ));
        let kotlin_fqn = self.resolve_class_fqn(&short);
        let throw_fn_name = format_ident!("throw_{}", short);

        // Collision guard: each exception's throw_<short> needs a unique
        // name to coexist as free fns in the same generated file. In
        // practice zenoh-java has one exception so this is just a guard.
        if self
            .exceptions
            .iter()
            .any(|e| e.throw_fn_name == throw_fn_name)
        {
            panic!(
                "kotlin_exception_class: another exception is already \
                 registered with Rust short name `{}` — rename the Rust \
                 type or chain `.kotlin_name(...)` after the first \
                 declaration to disambiguate",
                short
            );
        }

        self.exceptions.push(ExceptionConfig {
            rust_path: path,
            rust_short: short,
            kotlin_fqn,
            throw_fn_name,
            output_throws: Vec::new(),
        });
        let idx = self.exceptions.len() - 1;
        self.last_exception_idx = Some(idx);
        self.last_opaque_key = None;
        self.last_meta_key = None;
        self
    }

    /// Register an output converter that may raise a JVM exception.
    /// **Must be chained immediately after a [`Self::kotlin_exception_class`]
    /// call** — the registration is scoped to that exception, and the
    /// emitted converter's `Err` branch invokes that exception's
    /// `throw_<short>` function. The Kotlin `@Throws` annotation on the
    /// matching wrapper picks up the same exception's `kotlin_fqn`.
    ///
    /// `pattern` is the Rust type-key (with `_` placeholders for wildcards;
    /// arity 0..3). `builder` returns `Some((wire_type, body_expr))` where
    /// `body_expr` evaluates to `Result<wire_type, __JniErr>` (same shape
    /// as [`Self::output_wrapper`]) — the framework wraps this in
    /// `match { Ok(w) => w, Err(e) => { throw_<short>(&mut env, &e); sentinel } }`
    /// and emits a function returning the bare wire. The body may use `?`
    /// for sub-conversions and `return Err(...)` for custom failure
    /// detection (e.g. `i32 < 0` → error).
    pub fn output_throws<A, B>(mut self, pattern: impl AsRef<str>, builder: B) -> Self
    where
        B: WrapperBuilder<A>,
    {
        let idx = self.last_exception_idx.expect(
            "JniExt::output_throws must be chained immediately after a \
             `kotlin_exception_class` (and any `.kotlin_name(...)`) call — \
             the pattern is scoped to that exception",
        );
        let key = TypeKey::parse(pattern.as_ref());
        let rank = B::rank();
        self.exceptions[idx]
            .output_throws
            .push(ExceptionOutputThrows {
                pattern_key: key.clone(),
            });
        self.output_wrappers[rank].insert(key, builder.into_wrapper_fn());
        // `last_exception_idx` stays — multiple `output_throws` may
        // chain under the same exception. `last_*_key` are cleared
        // because the pattern itself isn't a Kotlin type registration.
        self.last_opaque_key = None;
        self.last_meta_key = None;
        self
    }
    /// Set the JVM/Kotlin base package (dot-separated, e.g.
    /// `"io.zenoh.jni"`). All derived forms (`java_class_prefix`,
    /// `jni_class_path`, `kotlin_callback_package`) are recomputed.
    pub fn package(mut self, p: impl Into<String>) -> Self {
        self.package = p.into().trim_matches('.').trim_matches('/').to_string();
        self.recompute_derived();
        self
    }
    /// Set the JVM class on which JNI `external fun`s live (e.g.
    /// `"JNINative"`). Affects `jni_class_path`.
    pub fn jni_native_class(mut self, c: impl Into<String>) -> Self {
        self.jni_native_class = c.into();
        self.recompute_derived();
        self
    }
    /// Set the leaf appended to [`Self::package`] for the auto-emitted
    /// callback fun-interface files (e.g. `"callbacks"`). Affects
    /// `kotlin_callback_package`.
    pub fn callback_subpackage(mut self, s: impl Into<String>) -> Self {
        self.callback_subpackage = s.into().trim_matches('.').to_string();
        self.recompute_derived();
        self
    }
    /// Set the prefix applied to auto-derived callback class names
    /// (default `"JNI"`).
    pub fn callback_name_prefix(mut self, s: impl Into<String>) -> Self {
        self.callback_name_prefix = s.into();
        self
    }
    /// Set the postfix applied to auto-derived callback class names
    /// (default `"Callback"`).
    pub fn callback_name_postfix(mut self, s: impl Into<String>) -> Self {
        self.callback_name_postfix = s.into();
        self
    }
    pub fn jni_method_suffix(mut self, s: impl Into<String>) -> Self {
        self.jni_method_suffix = s.into();
        self
    }

    /// Recompute the derived caches (`java_class_prefix`,
    /// `jni_class_path`, `kotlin_callback_package`) from
    /// (`package`, `jni_native_class`, `callback_subpackage`). Called by
    /// every setter that touches one of those source fields.
    fn recompute_derived(&mut self) {
        self.java_class_prefix = self.package.replace(".", "/");
        self.jni_class_path = if self.package.is_empty() {
            format!("Java_{}", self.jni_native_class)
        } else {
            format!(
                "Java_{}_{}",
                self.package.replace(".", "_"),
                self.jni_native_class
            )
        };
        self.kotlin_callback_package = if self.package.is_empty() {
            self.callback_subpackage.clone()
        } else if self.callback_subpackage.is_empty() {
            self.package.clone()
        } else {
            format!("{}.{}", self.package, self.callback_subpackage)
        };
    }

    /// Auto-derive a callback class name from the per-callback `stem`
    /// using the configured prefix/postfix (default `"JNI<stem>Callback"`).
    pub(crate) fn callback_class_name(&self, stem: &str) -> String {
        format!(
            "{}{}{}",
            self.callback_name_prefix, stem, self.callback_name_postfix
        )
    }

    /// Resolve a relative class name against [`Self::package`]. Panics
    /// if `name` contains a `.` (a check that catches accidental FQNs in
    /// the relative-name builders).
    pub(crate) fn resolve_class_fqn(&self, name: &str) -> String {
        assert!(
            !name.contains('.'),
            "Kotlin class name `{}` must be relative (no dots) — FQNs are derived from JniExt::package",
            name
        );
        if self.package.is_empty() {
            name.to_string()
        } else {
            format!("{}.{}", self.package, name)
        }
    }

    /// Resolve a relative callback class name against
    /// `package + "." + callback_subpackage`. Panics if `name` contains a `.`.
    pub(crate) fn resolve_callback_fqn(&self, name: &str) -> String {
        assert!(
            !name.contains('.'),
            "Kotlin callback name `{}` must be relative (no dots) — FQNs are derived from JniExt::package + callback_subpackage",
            name
        );
        if self.kotlin_callback_package.is_empty() {
            name.to_string()
        } else {
            format!("{}.{}", self.kotlin_callback_package, name)
        }
    }
    // ── Structured type-conversion builders ──────────────────────────

    /// Declare a typed Kotlin handle class backed by an opaque Rust
    /// type. Configures: jlong wire for both input and output,
    /// `Box::into_raw`/`Box::from_raw` lifecycle, the `instanceof`
    /// dispatch class, and the Kotlin typed-handle class FQN. By
    /// default a `.kt` shell is auto-emitted — chain
    /// [`Self::suppress_kotlin_code`] to keep the file hand-maintained,
    /// or chain one or more [`Self::method`] calls to promote
    /// `#[prebindgen]` functions onto the class as instance methods.
    pub fn kotlin_class(mut self, rust_key: impl AsRef<str>) -> Self {
        let key = TypeKey::parse(rust_key.as_ref());
        let short = rust_short_name(&key);
        let fqn = self.resolve_class_fqn(&short);
        let entry = self.types.entry(key.clone()).or_default();
        entry.opaque = Some(OpaqueConfig::default());
        // `kotlin_name` holds the typed-handle FQN for FQN-consumers
        // (typed-handle class emission, `instanceof` dispatch, return-
        // value constructor wrap). The value-context Kotlin name for
        // opaque types — `"Long"` — flows separately through
        // [`KotlinMeta::kotlin_name`] produced by the rank-0 opaque
        // handler, so wire-level mentions don't collide with the FQN.
        entry.kotlin_name = Some(fqn.clone());
        self.kotlin_type_fqns
            .push((key.as_str().to_string(), fqn));
        self.last_opaque_key = Some(key.clone());
        self.last_meta_key = Some(key);
        self.last_exception_idx = None;
        self
    }

    /// Promote a single `#[prebindgen]` function ident to an instance
    /// method on the Kotlin typed-handle class declared by the most
    /// recent [`Self::kotlin_class`] call. Chain multiple calls to add
    /// multiple methods. Panics if no kotlin_class is in scope.
    pub fn method(mut self, method: impl Into<String>) -> Self {
        let key = self.last_opaque_key.clone().expect(
            "JniExt::method must be chained immediately after a `kotlin_class` call",
        );
        let entry = self.types.get_mut(&key).expect("opaque entry vanished");
        let opaque = entry
            .opaque
            .as_mut()
            .expect("method on a non-opaque entry");
        opaque.methods.push(method.into());
        self
    }

    /// Opt out of Kotlin class emission for the most recent
    /// [`Self::kotlin_class`] — the `.kt` file is assumed to be
    /// hand-written. Without this, a typed-handle shell class is
    /// auto-emitted (plus any promoted [`Self::method`]s). Panics if
    /// no kotlin_class is in scope.
    pub fn suppress_kotlin_code(mut self) -> Self {
        let key = self.last_opaque_key.clone().expect(
            "JniExt::suppress_kotlin_code must be chained immediately after a `kotlin_class` call",
        );
        let entry = self.types.get_mut(&key).expect("opaque entry vanished");
        let opaque = entry
            .opaque
            .as_mut()
            .expect("suppress_kotlin_code on a non-opaque entry");
        opaque.suppress_kotlin_code = true;
        self
    }

    /// Register a custom rank-0 input converter for `rust_key`. The
    /// converter body sees `env: &mut JNIEnv` and `v: &<wire>` in
    /// scope and must produce a value of the Rust type.
    ///
    /// Chain [`Self::with_kotlin_name`] immediately after to bind the
    /// Kotlin type name when the wire isn't a primitive that
    /// `kotlin_for_wire` can resolve on its own (e.g.
    /// `jni::objects::JObject`).
    pub fn input_decoder(
        mut self,
        rust_key: impl AsRef<str>,
        wire_type: impl AsRef<str>,
        body_expr: impl AsRef<str>,
    ) -> Self {
        let key = TypeKey::parse(rust_key.as_ref());
        let wire: syn::Type = syn::parse_str(wire_type.as_ref())
            .unwrap_or_else(|e| panic!("input_decoder: invalid wire `{}`: {}", wire_type.as_ref(), e));
        let body: syn::Expr = syn::parse_str(body_expr.as_ref())
            .unwrap_or_else(|e| panic!("input_decoder: invalid body `{}`: {}", body_expr.as_ref(), e));
        let entry = self.types.entry(key.clone()).or_default();
        entry.input = Some(ConverterSpec {
            wire,
            body,
            niches: Niches::empty(),
        });
        self.last_opaque_key = None;
        self.last_meta_key = Some(key);
        self.last_exception_idx = None;
        self
    }

    /// Register a custom rank-0 output converter for `rust_key`. The
    /// converter body sees `env: &mut JNIEnv` and `v: <rust_type>` in
    /// scope and must produce a value of `wire_type`.
    ///
    /// Chain [`Self::with_kotlin_name`] immediately after to bind the
    /// Kotlin type name when the wire isn't a primitive that
    /// `kotlin_for_wire` can resolve on its own (e.g.
    /// `jni::sys::jobject`).
    pub fn output_encoder(
        mut self,
        rust_key: impl AsRef<str>,
        wire_type: impl AsRef<str>,
        body_expr: impl AsRef<str>,
    ) -> Self {
        let key = TypeKey::parse(rust_key.as_ref());
        let wire: syn::Type = syn::parse_str(wire_type.as_ref())
            .unwrap_or_else(|e| panic!("output_encoder: invalid wire `{}`: {}", wire_type.as_ref(), e));
        let body: syn::Expr = syn::parse_str(body_expr.as_ref())
            .unwrap_or_else(|e| panic!("output_encoder: invalid body `{}`: {}", body_expr.as_ref(), e));
        let entry = self.types.entry(key.clone()).or_default();
        entry.output = Some(ConverterSpec {
            wire,
            body,
            niches: Niches::empty(),
        });
        self.last_opaque_key = None;
        self.last_meta_key = Some(key);
        self.last_exception_idx = None;
        self
    }

    /// Stamp the Kotlin type name onto the entry registered by the
    /// most recent type-config builder. The relative `name` (no dots)
    /// resolves against [`Self::package`] for ordinary types and against
    /// `package + "." + callback_subpackage` for `impl Fn(...)` callbacks
    /// (detected via `entry.callback_kotlin_fqn` set by a prior
    /// [`Self::callback_input`]). When chained after
    /// [`Self::kotlin_exception_class`] the override updates that
    /// exception's `kotlin_fqn` instead — same resolution rules apply.
    /// Panics if `name` contains a `.` or if no per-type builder is in
    /// scope.
    pub fn kotlin_name(mut self, name: impl AsRef<str>) -> Self {
        // Exception-class override: takes precedence over the per-type
        // path because exception registrations don't go through
        // `self.types` (they live in `self.exceptions`).
        if let Some(idx) = self.last_exception_idx {
            let fqn = self.resolve_class_fqn(name.as_ref());
            self.exceptions[idx].kotlin_fqn = fqn;
            return self;
        }
        let key = self
            .last_meta_key
            .clone()
            .or_else(|| self.last_opaque_key.clone())
            .expect(
                "JniExt::kotlin_name must be chained immediately after a \
                 `kotlin_class` / `kotlin_value_type` / `input_decoder` / \
                 `output_encoder` / `callback_input` / `callback_kotlin_name` / \
                 `kotlin_exception_class` call",
            );
        let is_callback = self
            .types
            .get(&key)
            .map(|e| e.callback_kotlin_fqn.is_some())
            .unwrap_or(false);
        let fqn = if is_callback {
            self.resolve_callback_fqn(name.as_ref())
        } else {
            self.resolve_class_fqn(name.as_ref())
        };
        let entry = self.types.get_mut(&key).expect("meta entry vanished");
        entry.kotlin_name = Some(fqn.clone());
        if is_callback {
            entry.callback_kotlin_fqn = Some(fqn.clone());
        }
        // `kotlin_class` may have pushed a default `(rust_short_name)` FQN
        // for this key already; replace it so the JNI dispatcher's
        // `find_class` lookup picks up the override. `find()` in
        // `jobject_to_wire_adapter` returns the first match, so any
        // stale entry from `kotlin_class` would otherwise win.
        let key_str = key.as_str().to_string();
        self.kotlin_type_fqns.retain(|(k, _)| k != &key_str);
        self.kotlin_type_fqns.push((key_str, fqn));
        self
    }

    /// Stamp a verbatim Kotlin type expression (e.g. `"List<ByteArray>"`)
    /// onto the entry registered by the most recent type-config builder.
    /// Use this when the Kotlin type is not a class FQN (generics,
    /// primitives, container types). For class names, prefer
    /// [`Self::kotlin_name`] (relative + dot-checked).
    pub fn with_kotlin_type(mut self, kotlin_expr: impl Into<String>) -> Self {
        let key = self
            .last_meta_key
            .clone()
            .or_else(|| self.last_opaque_key.clone())
            .expect(
                "JniExt::with_kotlin_type must be chained immediately after a \
                 type-config builder",
            );
        let expr = kotlin_expr.into();
        let entry = self.types.get_mut(&key).expect("meta entry vanished");
        entry.kotlin_name = Some(expr.clone());
        self.kotlin_type_fqns
            .push((key.as_str().to_string(), expr));
        self
    }

    /// Sugar over [`Self::input_decoder`] for the common
    /// `jint → enum` pattern: `decode_path(*v)?` decodes the jint
    /// into the enum (or returns an error).
    pub fn jint_enum(
        self,
        rust_key: impl AsRef<str>,
        decode_path: impl AsRef<str>,
    ) -> Self {
        let body = format!("{}(*v)?", decode_path.as_ref());
        self.input_decoder(rust_key, "jni::sys::jint", &body)
    }

    /// Install a manual input converter for an `impl Fn(...)` callback
    /// parameter (`JObject` wire, body `<dispatcher_path>(env, &v)?`).
    /// The Kotlin FQN auto-derives via the callback-name template
    /// (`<callback_name_prefix><stem><callback_name_postfix>`); chain
    /// [`Self::kotlin_name`] immediately after to override with a
    /// relative name resolved against the callback subpackage.
    pub fn callback_input(
        mut self,
        impl_fn_key: impl AsRef<str>,
        dispatcher_path: impl AsRef<str>,
    ) -> Self {
        let key = TypeKey::parse(impl_fn_key.as_ref());
        let path: syn::Path = syn::parse_str(dispatcher_path.as_ref()).unwrap_or_else(|e| {
            panic!(
                "callback_input: invalid dispatcher path `{}`: {}",
                dispatcher_path.as_ref(),
                e
            )
        });
        let body: syn::Expr = syn::parse_quote!(#path(env, &v)?);
        let wire: syn::Type = syn::parse_quote!(jni::objects::JObject);
        let entry = self.types.entry(key.clone()).or_default();
        entry.input = Some(ConverterSpec {
            wire,
            body,
            niches: Niches::empty(),
        });
        // Marker so `kotlin_name` knows this entry is a callback and
        // resolves the relative name against the callback subpackage.
        // The actual FQN — until overridden — stays computed lazily by
        // `auto_callback_fqn` on read.
        entry.callback_kotlin_fqn = Some(String::new());
        self.last_opaque_key = None;
        self.last_meta_key = Some(key);
        self.last_exception_idx = None;
        self
    }

    /// Override the Kotlin FQN emitted for an `impl Fn(...)` callback
    /// without changing its Rust-side input converter. `name` is
    /// relative (no dots); resolves against
    /// `package + "." + callback_subpackage`.
    pub fn callback_kotlin_name(
        mut self,
        impl_fn_key: impl AsRef<str>,
        name: impl AsRef<str>,
    ) -> Self {
        let key = TypeKey::parse(impl_fn_key.as_ref());
        let fqn = self.resolve_callback_fqn(name.as_ref());
        let entry = self.types.entry(key.clone()).or_default();
        entry.callback_kotlin_fqn = Some(fqn.clone());
        entry.kotlin_name = Some(fqn.clone());
        self.kotlin_type_fqns
            .push((key.as_str().to_string(), fqn));
        self.last_opaque_key = None;
        self.last_meta_key = Some(key);
        self.last_exception_idx = None;
        self
    }

    /// Declare a Rust value type that should appear in Kotlin under a
    /// derived name. Default Kotlin name = Rust short name (generics /
    /// lifetimes stripped); chain [`Self::kotlin_name`] for an override
    /// or [`Self::with_kotlin_type`] for a verbatim type expression.
    /// Only affects Kotlin emission — no Rust-side converter override.
    pub fn kotlin_value_type(mut self, rust_key: impl AsRef<str>) -> Self {
        let key = TypeKey::parse(rust_key.as_ref());
        let short = rust_short_name(&key);
        let fqn = self.resolve_class_fqn(&short);
        let entry = self.types.entry(key.clone()).or_default();
        entry.kotlin_name = Some(fqn.clone());
        self.kotlin_type_fqns
            .push((key.as_str().to_string(), fqn));
        self.last_opaque_key = None;
        self.last_meta_key = Some(key);
        self.last_exception_idx = None;
        self
    }

    /// Register `impl Into<target>` source arms. `target_key` is the
    /// canonical Rust type (e.g. `"ZKeyExpr<'static>"`); `sources` is
    /// an ordered list of [`IntoSource`] arms (dispatch order matches
    /// iteration order).
    pub fn into_sources<I>(mut self, target_key: impl AsRef<str>, sources: I) -> Self
    where
        I: IntoIterator<Item = IntoSource>,
    {
        let key = TypeKey::parse(target_key.as_ref());
        self.into_sources_map
            .insert(key, sources.into_iter().collect());
        self.last_opaque_key = None;
        self.last_meta_key = None;
        self.last_exception_idx = None;
        self
    }

    /// Register a rank-N input wrapper. `pattern` contains 0–3 `_`
    /// placeholders; the closure's arity determines which rank table
    /// the entry lands in. The closure receives the wildcard
    /// substitutions and `&Registry<KotlinMeta>`, returning
    /// `Some((wire_type, body_expr))` or `None` (defer to the next
    /// resolver phase). The body sees `env: &mut JNIEnv` and
    /// `v: &<wire>` in scope.
    pub fn input_wrapper<A, B>(mut self, pattern: impl AsRef<str>, builder: B) -> Self
    where
        B: WrapperBuilder<A>,
    {
        let key = TypeKey::parse(pattern.as_ref());
        let rank = B::rank();
        self.input_wrappers[rank].insert(key, builder.into_wrapper_fn());
        self.last_opaque_key = None;
        self.last_exception_idx = None;
        self
    }

    /// Output-direction counterpart of [`Self::input_wrapper`]. The body
    /// returns `Result<wire, __JniErr>` and may use `?` for sub-conversions
    /// (the framework wraps it in `Ok(...)` automatically); see
    /// [`Self::output_throws`] for the variant that converts the `Err` arm
    /// into a JVM exception.
    pub fn output_wrapper<A, B>(mut self, pattern: impl AsRef<str>, builder: B) -> Self
    where
        B: WrapperBuilder<A>,
    {
        let key = TypeKey::parse(pattern.as_ref());
        let rank = B::rank();
        self.output_wrappers[rank].insert(key, builder.into_wrapper_fn());
        self.last_opaque_key = None;
        self.last_exception_idx = None;
        self
    }

    /// Find the registered exception that owns a particular
    /// `output_throws` pattern (by canonical [`TypeKey`]). Returns
    /// `None` for patterns not registered under any exception (those
    /// fall through to the plain `output_wrapper` path in
    /// [`Self::lookup_output_wrapper`]).
    pub(crate) fn exception_owning_pattern(
        &self,
        key: &TypeKey,
    ) -> Option<&ExceptionConfig> {
        self.exceptions
            .iter()
            .find(|e| e.output_throws.iter().any(|ot| &ot.pattern_key == key))
    }

    /// The primary (first-registered) exception, or `None` when no
    /// `kotlin_exception_class` has been declared yet. Consumed by
    /// [`Self::prerequisites`] (to splice `__JniErr`),
    /// [`Self::write_native_handle`] (Kotlin FQN), and various Kotlin
    /// emitters defaulting to the primary's throw fn.
    pub(crate) fn primary_exception(&self) -> Option<&ExceptionConfig> {
        self.exceptions.first()
    }

    // ── Wrapper-table lookups (used by PrebindgenExt impl) ───────────

    pub(crate) fn lookup_input_wrapper(
        &self,
        pat: &syn::Type,
        args: &[syn::Type],
        registry: &Registry<KotlinMeta>,
    ) -> Option<ConverterImpl<KotlinMeta>> {
        let rank = args.len();
        if rank > 3 {
            return None;
        }
        let key = TypeKey::from_type(pat);
        let builder = self.input_wrappers[rank].get(&key)?;
        let (wire, body) = builder(args, registry)?;
        // Reconstruct the full outer type by substituting `args` into the
        // wildcard slots of `pat` — the same shape the wrapper would have
        // had in the source.
        let outer = substitute_wildcards(pat, args);
        let niches = default_niches_for_wire(&wire);
        Some(ConverterImpl {
            function: self.build_input_fn(&outer, &wire, &body),
            destination: wire,
            niches,
            metadata: KotlinMeta::default(),
        })
    }

    /// Look up a registered output wrapper for `pat` with `args` substituted
    /// into its `_` slots.
    ///
    /// If the registration was created via [`Self::output_throws`], the
    /// emitted function returns the bare wire and the metadata carries
    /// `throws: Some(exception_fqn)`. Otherwise (plain [`Self::output_wrapper`])
    /// the emitted function returns `Result<wire, __JniErr>` and metadata
    /// carries no `throws`.
    pub(crate) fn lookup_output_wrapper(
        &self,
        pat: &syn::Type,
        args: &[syn::Type],
        registry: &Registry<KotlinMeta>,
    ) -> Option<ConverterImpl<KotlinMeta>> {
        let rank = args.len();
        if rank > 3 {
            return None;
        }
        let key = TypeKey::from_type(pat);
        let builder = self.output_wrappers[rank].get(&key)?;
        let (wire, body) = builder(args, registry)?;
        let outer = substitute_wildcards(pat, args);
        let niches = default_niches_for_wire(&wire);
        // Output-throws registration: emit a bare-wire-returning function
        // wrapping the body in match-throw, and stamp `throws` metadata.
        // For arity-1 wrappers the Kotlin projection of the wrapper is
        // the inner type's projection (e.g. `ZResult<T>` → `T`); inherit
        // `kotlin_name` from `args[0]`'s registered output entry and
        // record `args[0]`'s canonical key as `value_rust_key` so
        // downstream typed-handle / opaque-ctor lookups can find the
        // wrapped value's identity without baking a Result-shape into
        // the framework.
        if let Some(exc) = self.exception_owning_pattern(&key) {
            let throw_path = exception_throw_path(exc);
            let sentinel = sentinel_for_wire(&wire);
            let function = build_output_throws_fn(
                &outer,
                &wire,
                &body,
                &throw_path,
                &sentinel,
            );
            let inherited = if rank >= 1 {
                registry
                    .output_entry(&args[0])
                    .map(|e| (e.metadata.kotlin_name.clone(),
                              Some(TypeKey::from_type(&args[0]).as_str().to_string())))
                    .unwrap_or((None, None))
            } else {
                (
                    crate::jni::jni_kotlin_ext::kotlin_for_wire(&wire),
                    None,
                )
            };
            return Some(ConverterImpl {
                function,
                destination: wire,
                niches,
                metadata: KotlinMeta {
                    kotlin_name: inherited.0,
                    throws: Some(exc.kotlin_fqn.clone()),
                    throws_action: Some(throw_path),
                    value_rust_key: inherited.1,
                },
            });
        }
        Some(ConverterImpl {
            function: self.build_output_fn(&outer, &wire, &body),
            destination: wire,
            niches,
            metadata: KotlinMeta::default(),
        })
    }
}

/// Bare-ident path to the generated `throw_<short>` free function for
/// `exc` (e.g. `throw_ZError`). Spliced into wrapper code as a direct
/// call — `<path>(env, &err)` — so the trait/macro dance the legacy
/// `throw_exception!` indirection performed is replaced with a plain
/// function call. The path is unqualified because the throw fn lands
/// in the same generated file as every wrapper (emitted from
/// [`PrebindgenExt::prerequisites`]); same-module name resolution
/// finds it.
pub(crate) fn exception_throw_path(exc: &ExceptionConfig) -> syn::Path {
    let ident = exc.throw_fn_name.clone();
    syn::Path::from(ident)
}

/// Substitute the wildcard `_` slots of `pat` with `args` (left-to-right
/// depth-first), returning the concrete outer `syn::Type`. Mirrors the
/// substitution the resolver performs to derive a wildcard pattern from
/// a concrete type.
fn substitute_wildcards(pat: &syn::Type, args: &[syn::Type]) -> syn::Type {
    let mut idx = 0usize;
    fn walk(ty: &mut syn::Type, args: &[syn::Type], idx: &mut usize) {
        match ty {
            syn::Type::Infer(_) => {
                if let Some(replacement) = args.get(*idx) {
                    *ty = replacement.clone();
                }
                *idx += 1;
            }
            syn::Type::Path(tp) => {
                for seg in &mut tp.path.segments {
                    if let syn::PathArguments::AngleBracketed(ab) = &mut seg.arguments {
                        for arg in &mut ab.args {
                            if let syn::GenericArgument::Type(inner) = arg {
                                walk(inner, args, idx);
                            }
                        }
                    }
                }
            }
            syn::Type::Reference(r) => walk(&mut r.elem, args, idx),
            syn::Type::Tuple(t) => {
                for e in &mut t.elems {
                    walk(e, args, idx);
                }
            }
            syn::Type::Array(a) => walk(&mut a.elem, args, idx),
            syn::Type::Slice(s) => walk(&mut s.elem, args, idx),
            syn::Type::Ptr(p) => walk(&mut p.elem, args, idx),
            syn::Type::Paren(p) => walk(&mut p.elem, args, idx),
            syn::Type::Group(g) => walk(&mut g.elem, args, idx),
            _ => {}
        }
    }
    let mut out = pat.clone();
    walk(&mut out, args, &mut idx);
    out
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
    pub fn build_input_fn(
        &self,
        rust: &syn::Type,
        wire: &syn::Type,
        body: &syn::Expr,
    ) -> syn::ItemFn {
        let name = input_name(rust, wire);
        let rust_with_lifetime = annotate_borrow_with_lifetime(rust, "env");
        let wire_with_lifetime = annotate_jobject_with_lifetime(wire, "v");
        if matches!(wire, syn::Type::Ptr(_)) {
            syn::parse_quote!(
                #[allow(non_snake_case, unused_mut, unused_variables, unused_braces, dead_code)]
                pub(crate) unsafe fn #name<'env>(env: &mut jni::JNIEnv<'env>, v: #wire) -> ::core::result::Result<#rust_with_lifetime, __JniErr> {
                    Ok(#body)
                }
            )
        } else {
            syn::parse_quote!(
                #[allow(non_snake_case, unused_mut, unused_variables, unused_braces, dead_code)]
                pub(crate) unsafe fn #name<'env, 'v>(env: &mut jni::JNIEnv<'env>, v: &#wire_with_lifetime) -> ::core::result::Result<#rust_with_lifetime, __JniErr> {
                    Ok(#body)
                }
            )
        }
    }

    /// Build the standard JNI output-converter `fn`. Body assumes in-scope
    /// `env: &mut JNIEnv` and `v: <rust>` (by value — handles like
    /// `Subscriber<()>` aren't `Clone`, so callers move into the converter).
    pub fn build_output_fn(
        &self,
        rust: &syn::Type,
        wire: &syn::Type,
        body: &syn::Expr,
    ) -> syn::ItemFn {
        let name = output_name(rust, wire);
        let wire_with_lifetime = annotate_jobject_with_lifetime(wire, "a");
        syn::parse_quote!(
            #[allow(non_snake_case, unused_mut, unused_variables, unused_braces, dead_code)]
            pub(crate) unsafe fn #name<'a>(env: &mut jni::JNIEnv<'a>, v: #rust) -> ::core::result::Result<#wire_with_lifetime, __JniErr> {
                Ok(#body)
            }
        )
    }


    /// Universal "opaque Box-handle as `jlong`" pair — input side.
    ///
    /// Use for any Rust type whose lifecycle is owned by the Java side:
    /// Java holds the raw `Box<T>` pointer as a `Long` and calls Rust
    /// passing the pointer. The converter handles both parameter
    /// shapes, the decision is taken in `on_function` from the
    /// parameter's syntax:
    ///
    /// **`&T` sites (borrow)**: `OwnedObject::from_raw` stores the
    /// pointer without taking ownership of the `Box`; `Deref<Target
    /// = T>` exposes `&*ptr` so the generated call site can borrow it
    /// as `&T`. The wrapper has no `Drop` — nothing is freed, the
    /// heap allocation stays with Java. The Java side must take the
    /// pointer out of its `NativeHandle.withPtr` (read lock) so the
    /// borrow is sequenced against any concurrent consume / close.
    ///
    /// **`T` sites (consume, by-value)**: the call-site emitter
    /// bypasses `OwnedObject` and inlines `*Box::from_raw(ptr)` —
    /// infallible. The Java side must take the pointer out of its
    /// `NativeHandle.consume` (write lock + atomic null) before
    /// invoking this entry point; that write lock drains concurrent
    /// borrows and the atomic-null ensures the same Long cannot be
    /// passed twice. No `T: Clone` bound (Box requires nothing of T),
    /// so non-Clone handles (`Publisher<'a>`, `Subscriber<()>`) can
    /// consume.
    ///
    /// **Convention** (single rule for both input and output):
    /// * Wire: `jni::sys::jlong` — the same width JNI hands across
    ///   the boundary on every platform (`*mut T` would mismatch
    ///   on 32-bit, where ptr size is 4 but jlong is 8).
    /// * Output: `Box::into_raw(Box::new(v)) as i64` — leak the heap
    ///   allocation to Java; sole owner is whoever later calls
    ///   `Box::from_raw` on the same pointer.
    /// * Input: `OwnedObject::from_raw(*v as *const T)` (borrow only).
    /// * Niche: `0i64` / `*v == 0` — `Box::into_raw` never returns 0,
    ///   so `Option<T>` automatically synthesises `0` = `None`,
    ///   matching the legacy "null pointer" ABI for nullable handles.
    pub fn opaque_handle_input(&self, ty: &syn::Type) -> ConverterImpl<KotlinMeta> {
        let wire: syn::Type = syn::parse_quote!(jni::sys::jlong);
        let name = input_name(ty, &wire);
        let function: syn::ItemFn = syn::parse_quote!(
            #[allow(non_snake_case, unused_mut, unused_variables, unused_braces, dead_code)]
            pub(crate) unsafe fn #name<'env, 'v>(
                env: &mut jni::JNIEnv<'env>,
                v: &jni::sys::jlong,
            ) -> ::core::result::Result<OwnedObject<#ty>, __JniErr> {
                Ok(unsafe { OwnedObject::from_raw(*v as *const #ty) })
            }
        );
        ConverterImpl {
            function,
            destination: wire,
            niches: Niches::one(
                syn::parse_quote!(0i64),
                syn::parse_quote!(*v == 0),
            ),
            // Opaque handles' value-context Kotlin name — `"Long"`.
            // The typed-handle FQN lives in [`OpaqueConfig::fqn`] and is
            // consulted by FQN-specific paths (typed-handle class
            // emission, `instanceof` dispatch, return-value constructor
            // wrap) rather than via metadata.
            metadata: KotlinMeta::from_name("Long"),
        }
    }

    /// If the user pinned a Kotlin name for `outer_ty` via
    /// [`Self::kotlin_value_type`] (or it's an opaque-handle entry that
    /// kept its FQN in `kotlin_name`), use that name; otherwise leave
    /// the auto-derived `inherited` value untouched. Lets handler arms
    /// inherit by default but yield to an explicit user pin when one
    /// exists — same precedence the legacy `KotlinTypeMap.lookup`
    /// fallback chain had.
    pub(crate) fn override_kotlin_name(
        &self,
        outer_ty: &syn::Type,
        inherited: Option<String>,
    ) -> Option<String> {
        let key = TypeKey::from_type(outer_ty);
        if let Some(cfg) = self.types.get(&key) {
            // Opaque-handle entries keep their typed FQN in
            // `kotlin_name` for FQN-consumers, but the value-context
            // name is `"Long"` (set on the rank-0 handler's metadata).
            // Don't let that FQN leak into a wrapper's metadata.
            if cfg.opaque.is_none() {
                if let Some(name) = &cfg.kotlin_name {
                    return Some(name.clone());
                }
            }
        }
        inherited
    }

    /// Auto-derived Kotlin FQN for an `impl Fn(args)` callback. Same
    /// convention `collect_kotlin_callback_fqns` uses, exposed here so
    /// the rank-0/rank-1 callback dispatcher can stamp the FQN into
    /// the converter's [`KotlinMeta`] at creation time.
    pub(crate) fn auto_callback_fqn(&self, args: &[syn::Type]) -> String {
        let stem = crate::jni::jni_kotlin_ext::derive_callback_stem(args);
        let class = self.callback_class_name(&stem);
        if self.kotlin_callback_package.is_empty() {
            class
        } else {
            format!("{}.{}", self.kotlin_callback_package, class)
        }
    }

    /// Canonical input-converter name for `(rust, wire)` — exposed
    /// for plugin wrapper exts that build `ConverterImpl::function`
    /// manually with a non-standard return type (e.g.
    /// `impl Into<…>` parameters that can't be expressed via
    /// [`Self::input_wrapper`]'s fixed signature shape).
    pub fn input_converter_name(&self, rust: &syn::Type, wire: &syn::Type) -> syn::Ident {
        input_name(rust, wire)
    }

    /// Symmetric to [`Self::input_converter_name`].
    pub fn output_converter_name(&self, rust: &syn::Type, wire: &syn::Type) -> syn::Ident {
        output_name(rust, wire)
    }

    /// Output side of [`Self::opaque_handle_input`] — see that method's
    /// docs for the full convention.
    pub fn opaque_handle_output(&self, ty: &syn::Type) -> ConverterImpl<KotlinMeta> {
        let wire: syn::Type = syn::parse_quote!(jni::sys::jlong);
        let body: syn::Expr = syn::parse_quote!(
            std::boxed::Box::into_raw(std::boxed::Box::new(v)) as i64
        );
        ConverterImpl {
            function: self.build_output_fn(ty, &wire, &body),
            destination: wire,
            niches: Niches::one(
                syn::parse_quote!(0i64),
                syn::parse_quote!(*v == 0),
            ),
            // Opaque handles' value-context Kotlin name — see
            // [`Self::opaque_handle_input`].
            metadata: KotlinMeta::from_name("Long"),
        }
    }

    /// Emit the JObject-typed dispatching input converter for
    /// `impl Into<target> + Send + 'static` given an already-assembled
    /// source list. The caller — typically a
    /// [`PrebindgenExt::dispatch_into_input`] implementation —
    /// supplies every arm explicitly (including the identity arm
    /// `target → target` if wanted) with each source's borrow/consume
    /// mode.
    ///
    /// Emits an `instanceof` chain over each source `S`: every arm
    /// calls `S`'s already-registered input decoder (wire-narrowed
    /// from the parameter's `JObject`) and converts to `target` via
    /// `TryInto`, so both `From<S> for target` (zero-cost) and
    /// `TryFrom<S> for target` (fallible) work uniformly.
    ///
    /// Per-source mode handling (only relevant for opaque sources —
    /// non-opaque sources have no `Box` slot, so mode is moot):
    /// * [`IntoSourceMode::Borrow`] → decode via
    ///   `OwnedObject::from_raw(...).clone()`. Java's `Box` slot stays
    ///   live; requires `T: Clone`.
    /// * [`IntoSourceMode::Consume`] → bypass `OwnedObject` and inline
    ///   `*Box::from_raw(ptr as *mut T)`. Java's `Box` slot is taken;
    ///   the caller's typed handle must be invalidated (the Kotlin
    ///   wrapper does this via `NativeHandle.consume`). No `T: Clone`
    ///   bound.
    ///
    /// Returns `None` when `sources` is empty or any source lacks a
    /// registered input decoder; the resolver iterates to a fixed
    /// point and will retry on a later round once all decoders exist.
    pub fn emit_into_dispatcher(
        &self,
        target: &syn::Type,
        sources: &[IntoSource],
        registry: &Registry<KotlinMeta>,
    ) -> Option<ConverterImpl<KotlinMeta>> {
        if sources.is_empty() {
            return None;
        }
        let target_key = TypeKey::from_type(target).as_str().to_string();

        let mut arms: Vec<TokenStream> = Vec::with_capacity(sources.len());
        for src in sources {
            let src_ty = &src.source_type;
            let src_key = TypeKey::from_type(src_ty).as_str().to_string();
            let src_entry = registry.input_entry(src_ty)?;
            let decoder = src_entry.function.sig.ident.clone();
            let wire = src_entry.destination.clone();
            let (java_class, prelude, decoded_ref) =
                jobject_to_wire_adapter(&wire, src_ty, &self.kotlin_type_fqns).unwrap_or_else(
                    || {
                        panic!(
                            "emit_into_dispatcher: source `{}` has wire `{}` which is not a \
                             supported Into-source wire shape (target = `{}`)",
                            src_key,
                            wire.to_token_stream(),
                            target_key
                        )
                    },
                );
            // Opaque sources branch on the declared mode. Non-opaque
            // sources don't own a `Box` slot, so they just decode
            // normally and `mode` has no effect on the emitted code.
            let is_opaque = converter_returns_owned_object(&src_entry.function.sig.output);
            let decode_expr: syn::Expr = if is_opaque {
                match src.mode {
                    // Method-call `.clone()` triggers method auto-deref:
                    // OwnedObject<T> has no Clone impl, so dispatch
                    // derefs to `&T` and calls `T::clone`. Requires
                    // `T: Clone`. Java's `Box` slot stays live.
                    IntoSourceMode::Borrow => syn::parse_quote!(
                        unsafe { #decoder(env, #decoded_ref)? }.clone()
                    ),
                    // Bypass the decoder entirely: reconstruct the
                    // unique `Box<T>` from Java's pointer and move `T`
                    // out, freeing the heap allocation. Mirrors the
                    // direct-by-value consume codegen at
                    // `emit_jni_function_wrapper`. Unique-ownership
                    // invariant is upheld by `NativeHandle.consume`
                    // (write lock + atomic null) on the Kotlin side.
                    // `#decoded_ref` is `&__narrowed` for jlong wires;
                    // dereference to recover the `jlong` value.
                    IntoSourceMode::Consume => syn::parse_quote!(
                        unsafe { *std::boxed::Box::from_raw(*#decoded_ref as *mut #src_ty) }
                    ),
                }
            } else {
                syn::parse_quote!(unsafe { #decoder(env, #decoded_ref)? })
            };
            arms.push(quote! {
                {
                    let __class = env
                        .find_class(#java_class)
                        .map_err(|e| <__JniErr as ::core::convert::From<String>>::from(format!("find {}: {}", #java_class, e)))?;
                    let __is = env
                        .is_instance_of(v, &__class)
                        .map_err(|e| <__JniErr as ::core::convert::From<String>>::from(format!("instanceof {}: {}", #java_class, e)))?;
                    if __is {
                        #prelude
                        let __decoded: #src_ty = #decode_expr;
                        let __converted: #target = ::core::convert::TryInto::try_into(__decoded)
                            .map_err(|e| <__JniErr as ::core::convert::From<String>>::from(format!(
                                "convert {} -> {}: {}", #src_key, #target_key, e)))?;
                        return Ok(__converted);
                    }
                }
            });
        }

        let wire: syn::Type = syn::parse_quote!(jni::objects::JObject);
        let pat: syn::Type = syn::parse_quote!(impl Into<#target> + Send + 'static);
        let name = input_name(&pat, &wire);
        let target_label = target_key.clone();
        let function: syn::ItemFn = syn::parse_quote!(
            #[allow(non_snake_case, unused_mut, unused_variables, unused_braces, dead_code)]
            pub(crate) unsafe fn #name<'env, 'v>(
                env: &mut jni::JNIEnv<'env>,
                v: &jni::objects::JObject<'v>,
            ) -> ::core::result::Result<#target, __JniErr> {
                #(#arms)*
                Err(<__JniErr as ::core::convert::From<String>>::from(format!(
                    "impl Into<{}>: no matching source arm for runtime class", #target_label)))
            }
        );

        Some(ConverterImpl {
            function,
            destination: wire,
            niches: Niches::empty(),
            // `impl Into<T>` parameters surface as Kotlin `Any` — the
            // safe wrapper does an `is JNI<X>` chain on the value, and
            // the JNI dispatcher's matching arm uses each source's
            // typed FQN under the hood.
            metadata: KotlinMeta::from_name("Any"),
        })
    }

}

/// One `pub(crate) fn throw_<short>(...)` item for an exception.
/// Emitted from [`PrebindgenExt::prerequisites`] so it lands at the
/// top of the same generated file as every other converter — wrapper
/// code below can call it by bare name (`throw_<short>(env, &err)`);
/// hand-written modules in the binding crate reach it via the include
/// module path (e.g. `crate::generated::throw_<short>`). The body
/// matches the legacy hand-written `impl ThrowOnJvm for ZError`
/// exactly (find_class via slash-form FQN, throw_new with
/// `err.to_string()`, `tracing::error!` on either failure) — same
/// runtime behaviour, fewer indirections.
fn build_throw_fn_item(exc: &ExceptionConfig) -> syn::Item {
    let throw_fn = &exc.throw_fn_name;
    let rust_ty = &exc.rust_path;
    let class_path_slashes = exc.kotlin_fqn.replace('.', "/");
    syn::parse_quote!(
        #[allow(non_snake_case)]
        pub(crate) fn #throw_fn(
            env: &mut jni::JNIEnv,
            err: &#rust_ty,
        ) {
            let exception_class = match env.find_class(#class_path_slashes) {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("Failed to retrieve exception class: {}", e);
                    return;
                }
            };
            if let Err(e) = env.throw_new(exception_class, err.to_string()) {
                tracing::error!("Failed to throw exception: {}", e);
            }
        }
    )
}

// ──────────────────────────────────────────────────────────────────────
// PrebindgenExt impl
// ──────────────────────────────────────────────────────────────────────

impl PrebindgenExt for JniExt {
    /// Cross-language extras every JNI converter carries — currently
    /// the Kotlin value-context type name. Filled by the rank-N
    /// handlers at the same point they build the wire/body; the
    /// resolver propagates it into [`crate::core::registry::TypeEntry::metadata`];
    /// the Kotlin emitter reads it back to drive every wrapper /
    /// typed-handle / `JNIWrappers` signature.
    type Metadata = KotlinMeta;

    /// Emit the `OwnedObject<T>` borrow wrapper used by
    /// [`Self::opaque_handle_input`] into the destination file.
    /// The struct is referenced by an unqualified `OwnedObject` from
    /// the same generated file, so no `use` paths leak into the host
    /// crate's source tree.
    fn prerequisites(&self) -> Vec<syn::Item> {
        let primary = self.primary_exception().unwrap_or_else(|| panic!(
            "JniExt: no exception class registered — call \
             `.kotlin_exception_class(\"path::to::ErrorType\")` at least once \
             (the primary error type must implement `From<String>`)"
        ));
        let error_type = &primary.rust_path;
        // Single splice point for the configured error type: every generated
        // converter signature and internal-failure construction site refers to
        // the `__JniErr` alias instead of the concrete path.
        let alias: syn::Item = syn::parse_quote!(
            #[allow(dead_code)]
            pub(crate) type __JniErr = #error_type;
        );
        let mut items = vec![alias];
        items.extend(owned_object_prerequisite_items());
        // Throw fns — one `pub(crate) fn throw_<short>(env, &err)` per
        // registered `kotlin_exception_class`. Emitted as prerequisites
        // (above the converters) so the wrappers below can reference
        // them by bare name; the binding crate references them as
        // `<include_module>::throw_<short>` from outside the file.
        items.extend(self.exceptions.iter().map(build_throw_fn_item));
        items
    }

    // ── Item methods ─────────────────────────────────────────────────

    fn on_function(&self, f: &syn::ItemFn, registry: &Registry<KotlinMeta>) -> TokenStream {
        emit_jni_function_wrapper(self, f, registry)
    }

    fn on_struct(&self, _s: &syn::ItemStruct, _registry: &Registry<KotlinMeta>) -> TokenStream {
        // Struct converter bodies are emitted by the resolver via
        // on_input_type_rank_0 / on_output_type_rank_0 below; no separate
        // per-struct item is needed.
        TokenStream::new()
    }

    fn on_enum(&self, _e: &syn::ItemEnum, _registry: &Registry<KotlinMeta>) -> TokenStream {
        TokenStream::new()
    }

    fn on_const(&self, c: &syn::ItemConst, _registry: &Registry<KotlinMeta>) -> TokenStream {
        c.to_token_stream()
    }

    // ── Input converters ─────────────────────────────────────────────

    fn on_input_type_rank_0(
        &self,
        ty: &syn::Type,
        registry: &Registry<KotlinMeta>,
    ) -> Option<ConverterImpl<KotlinMeta>> {
        // Structured-config overrides first (opaque handles, custom decoders).
        let key = TypeKey::from_type(ty);
        if let Some(cfg) = self.types.get(&key) {
            if cfg.opaque.is_some() {
                return Some(self.opaque_handle_input(ty));
            }
            if let Some(spec) = &cfg.input {
                let kotlin_name = cfg
                    .kotlin_name
                    .clone()
                    .or_else(|| crate::jni::jni_kotlin_ext::kotlin_for_wire(&spec.wire));
                return Some(ConverterImpl {
                    function: self.build_input_fn(ty, &spec.wire, &spec.body),
                    destination: spec.wire.clone(),
                    niches: spec.niches.clone(),
                    metadata: KotlinMeta { kotlin_name, throws: None, throws_action: None, value_rust_key: None },
                });
            }
        }
        if let Some((wire, body)) = primitive_input(ty) {
            let niches = default_niches_for_wire(&wire);
            let kotlin_name = crate::jni::jni_kotlin_ext::kotlin_for_wire(&wire);
            return Some(ConverterImpl {
                function: self.build_input_fn(ty, &wire, &body),
                destination: wire,
                niches,
                metadata: KotlinMeta { kotlin_name, throws: None, throws_action: None, value_rust_key: None },
            });
        }
        if let Some(name) = bare_path_ident(ty) {
            if let Some((s, _)) = registry.structs.get(&name) {
                let (wire, body) = struct_input_body(self, s, registry)?;
                let niches = default_niches_for_wire(&wire);
                // Auto-generated struct: the value-context Kotlin name is
                // whatever the user pinned via `kotlin_value_type`. If
                // they didn't, leave `kotlin_name = None` — emitter
                // surfaces this as a build-time hard error.
                let kotlin_name = self.types.get(&key).and_then(|c| c.kotlin_name.clone());
                return Some(ConverterImpl {
                    function: self.build_input_fn(ty, &wire, &body),
                    destination: wire,
                    niches,
                    metadata: KotlinMeta { kotlin_name, throws: None, throws_action: None, value_rust_key: None },
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
        registry: &Registry<KotlinMeta>,
    ) -> Option<ConverterImpl<KotlinMeta>> {
        if let Some(conv) = self.lookup_input_wrapper(pat, &[t1.clone()], registry) {
            return Some(conv);
        }
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
            let outer_ty: syn::Type = syn::parse_quote!(&#t1);
            // `&T` is a Kotlin-side no-op — inherit the inner type's
            // name, unless the user pinned an explicit override on
            // `&T` itself (rare but legal).
            let kotlin_name = self.override_kotlin_name(
                &outer_ty,
                inner.metadata.kotlin_name.clone(),
            );
            return Some(ConverterImpl {
                destination: inner.destination.clone(),
                function: inner.function.clone(),
                niches: inner.niches.clone(),
                metadata: KotlinMeta { kotlin_name, throws: None, throws_action: None, value_rust_key: None },
            });
        }
        if pat_match(pat, "Option < _ >") {
            let outer_ty: syn::Type = syn::parse_quote!(Option<#t1>);
            let (wire, body, niches) = option_input(t1, registry)?;
            // Inherit the inner's name; user pins on `Option<T>` win.
            // The nullability marker (`?`) is added by the use site.
            let inherited = registry
                .input_entry(t1)
                .and_then(|e| e.metadata.kotlin_name.clone());
            let kotlin_name = self.override_kotlin_name(&outer_ty, inherited);
            return Some(ConverterImpl {
                function: self.build_input_fn(&outer_ty, &wire, &body),
                destination: wire,
                niches,
                metadata: KotlinMeta { kotlin_name, throws: None, throws_action: None, value_rust_key: None },
            });
        }
        None
    }

    fn dispatch_into_input(
        &self,
        target: &syn::Type,
        sources: &[IntoSource],
        registry: &Registry<KotlinMeta>,
    ) -> Option<ConverterImpl<KotlinMeta>> {
        self.emit_into_dispatcher(target, sources, registry)
    }

    fn dispatch_fn_input(
        &self,
        args: &[syn::Type],
        registry: &Registry<KotlinMeta>,
    ) -> Option<ConverterImpl<KotlinMeta>> {
        let outer_ty = build_fn_type(args);
        let (wire, body) = callback_input(self, args, registry)?;
        let niches = default_niches_for_wire(&wire);
        // Kotlin sees `impl Fn(...)` as the matching `JNI<Stem>Callback`
        // fun-interface (or the user-overridden FQN). Use the override
        // when set; fall back to the auto-derived stem.
        let outer_key = TypeKey::from_type(&outer_ty);
        let kotlin_name = self
            .types
            .get(&outer_key)
            .and_then(|c| c.callback_kotlin_fqn.clone())
            .or_else(|| Some(self.auto_callback_fqn(args)));
        Some(ConverterImpl {
            function: self.build_input_fn(&outer_ty, &wire, &body),
            destination: wire,
            niches,
            metadata: KotlinMeta { kotlin_name, throws: None, throws_action: None, value_rust_key: None },
        })
    }

    fn on_input_type_rank_2(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        t2: &syn::Type,
        registry: &Registry<KotlinMeta>,
    ) -> Option<ConverterImpl<KotlinMeta>> {
        let _ = registry;
        self.lookup_input_wrapper(pat, &[t1.clone(), t2.clone()], registry)
    }

    fn on_input_type_rank_3(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        t2: &syn::Type,
        t3: &syn::Type,
        registry: &Registry<KotlinMeta>,
    ) -> Option<ConverterImpl<KotlinMeta>> {
        let _ = registry;
        self.lookup_input_wrapper(pat, &[t1.clone(), t2.clone(), t3.clone()], registry)
    }

    // ── Output converters ────────────────────────────────────────────

    fn on_output_type_rank_0(
        &self,
        ty: &syn::Type,
        registry: &Registry<KotlinMeta>,
    ) -> Option<ConverterImpl<KotlinMeta>> {
        // Structured-config overrides first (opaque handles, custom encoders).
        let key = TypeKey::from_type(ty);
        if let Some(cfg) = self.types.get(&key) {
            if cfg.opaque.is_some() {
                return Some(self.opaque_handle_output(ty));
            }
            if let Some(spec) = &cfg.output {
                let kotlin_name = cfg
                    .kotlin_name
                    .clone()
                    .or_else(|| crate::jni::jni_kotlin_ext::kotlin_for_wire(&spec.wire));
                return Some(ConverterImpl {
                    function: self.build_output_fn(ty, &spec.wire, &spec.body),
                    destination: spec.wire.clone(),
                    niches: spec.niches.clone(),
                    metadata: KotlinMeta { kotlin_name, throws: None, throws_action: None, value_rust_key: None },
                });
            }
        }
        // `()` — identity converter so `fn foo()` and `fn foo() -> ()`
        // funnel through the same uniform output path as everything else.
        // Wire is `()`. Body just returns `v`. No Kotlin name — Unit
        // returns are dropped from emitted signatures, so metadata stays
        // empty.
        if pat_match(ty, "()") {
            let wire: syn::Type = syn::parse_quote!(());
            let body: syn::Expr = syn::parse_quote!(v);
            return Some(ConverterImpl {
                function: self.build_output_fn(ty, &wire, &body),
                destination: wire,
                niches: Niches::empty(),
                metadata: KotlinMeta::default(),
            });
        }
        if let Some((wire, body)) = primitive_output(ty) {
            let niches = default_niches_for_wire(&wire);
            let kotlin_name = crate::jni::jni_kotlin_ext::kotlin_for_wire(&wire);
            return Some(ConverterImpl {
                function: self.build_output_fn(ty, &wire, &body),
                destination: wire,
                niches,
                metadata: KotlinMeta { kotlin_name, throws: None, throws_action: None, value_rust_key: None },
            });
        }
        if let Some(name) = bare_path_ident(ty) {
            if let Some((s, _)) = registry.structs.get(&name) {
                let (wire, body) = struct_output_body(self, s, registry)?;
                let niches = default_niches_for_wire(&wire);
                let kotlin_name = self.types.get(&key).and_then(|c| c.kotlin_name.clone());
                return Some(ConverterImpl {
                    function: self.build_output_fn(ty, &wire, &body),
                    destination: wire,
                    niches,
                    metadata: KotlinMeta { kotlin_name, throws: None, throws_action: None, value_rust_key: None },
                });
            }
        }
        None
    }

    fn on_output_type_rank_1(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        registry: &Registry<KotlinMeta>,
    ) -> Option<ConverterImpl<KotlinMeta>> {
        if let Some(conv) = self.lookup_output_wrapper(pat, &[t1.clone()], registry) {
            return Some(conv);
        }
        // No built-in Result/ZResult special-case: binding registers any
        // throw-on-Err behavior via `JniExt::output_throws("ZResult < _ >", ...)`,
        // which routes through `lookup_output_wrapper` above.
        if pat_match(pat, "Option < _ >") {
            let outer_ty: syn::Type = syn::parse_quote!(Option<#t1>);
            let (wire, body, niches) = option_output(t1, registry)?;
            let inherited = registry
                .output_entry(t1)
                .and_then(|e| e.metadata.kotlin_name.clone());
            let kotlin_name = self.override_kotlin_name(&outer_ty, inherited);
            return Some(ConverterImpl {
                function: self.build_output_fn(&outer_ty, &wire, &body),
                destination: wire,
                niches,
                metadata: KotlinMeta { kotlin_name, throws: None, throws_action: None, value_rust_key: None },
            });
        }
        None
    }

    fn on_output_type_rank_2(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        t2: &syn::Type,
        registry: &Registry<KotlinMeta>,
    ) -> Option<ConverterImpl<KotlinMeta>> {
        self.lookup_output_wrapper(pat, &[t1.clone(), t2.clone()], registry)
    }

    fn on_output_type_rank_3(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        t2: &syn::Type,
        t3: &syn::Type,
        registry: &Registry<KotlinMeta>,
    ) -> Option<ConverterImpl<KotlinMeta>> {
        self.lookup_output_wrapper(pat, &[t1.clone(), t2.clone(), t3.clone()], registry)
    }

    fn into_sources(&self, target: &syn::Type) -> Vec<IntoSource> {
        let key = TypeKey::from_type(target);
        self.into_sources_map
            .get(&key)
            .cloned()
            .unwrap_or_default()
    }
}

// ──────────────────────────────────────────────────────────────────────
// Function-wrapper emission (JNI extern "C")
// ──────────────────────────────────────────────────────────────────────

fn emit_jni_function_wrapper(ext: &JniExt, f: &syn::ItemFn, registry: &Registry<KotlinMeta>) -> TokenStream {
    let original_ident = &f.sig.ident;
    let wrapper_ident = mangle_jni_name(ext, original_ident);
    let source_module = &ext.source_module;

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

        // By-value `T` opaque-handle parameter: emit the consume
        // converter inline, bypassing `OwnedObject`. The Java side
        // takes the pointer out of its `NativeHandle.consume` under
        // the write lock and passes it here; `Box::from_raw`
        // reconstructs the unique owner and `*box` moves `T` out,
        // dropping the heap allocation. The unique-ownership
        // invariant is upheld by `NativeHandle.consume` (write-lock
        // + atomic pointer take), which drains all in-flight borrows
        // and ensures no live borrow can outlive this point. No
        // `T: Clone` bound, so non-Clone handles (e.g. `Publisher<'a>`)
        // work too.
        let is_consume = !matches!(arg_ty, syn::Type::Reference(_))
            && converter_returns_owned_object(&entry.function.sig.output);
        if is_consume {
            wire_params.push(quote!(#wire_ident: jni::sys::jlong));
            prelude.push(quote!(
                let #arg_ident: #arg_ty = unsafe {
                    *std::boxed::Box::from_raw(#wire_ident as *mut #arg_ty)
                };
            ));
            call_args.push(quote!(#arg_ident));
            continue;
        }

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

    // Output: look up the registered converter for the return type as
    // written. If it was registered via `JniExt::output_throws`, it returns
    // the bare wire and owns its internal error handling; the wrapper
    // additionally uses the same throw action to handle input-decode `?`
    // failures (which the converter never sees — those are wrapper-internal).
    // If it's a plain `output_wrapper` registration (returns
    // `Result<wire, __JniErr>`), the wrapper unwraps with a sentinel on Err
    // (configuration bug if reached).
    let return_ty: syn::Type = match &f.sig.output {
        syn::ReturnType::Default => syn::parse_quote!(()),
        syn::ReturnType::Type(_, ty) => (**ty).clone(),
    };
    let output_entry = registry.output_entry(&return_ty).unwrap_or_else(|| {
        panic!(
            "JniExt::on_function: return type `{}` of `{}` has no registered output \
             converter — register one via `JniExt::output_wrapper` / `JniExt::output_throws`",
            TypeKey::from_type(&return_ty),
            original_ident,
        )
    });
    let wire_return_ty = output_entry.destination.clone();
    let conv = output_entry.function.sig.ident.clone();
    let wire_with_lifetime = annotate_jobject_with_lifetime(&wire_return_ty, "a");
    let wire_return = wire_with_lifetime.to_token_stream();
    let on_err: TokenStream = sentinel_for_wire(&wire_return_ty);

    let call_expr = quote!(#source_module::#original_ident(#(#call_args),*));

    // The throw action for wrapper-internal failures (input-decode `?`)
    // comes from the same `output_throws` registration the return-type
    // converter is bound to — no separate global throw config. The
    // converter's `KotlinMeta.throws_action` carries the path to the
    // generated `throw_<short>` free function emitted by
    // [`JniExt::build_exception_items`]; we splice a direct call to it
    // (no macro indirection) so the generated file stays self-contained.
    let body = if let Some(throw_fn) = output_entry.metadata.throws_action.clone() {
        // Throwing converter: wrapper-internal `?` failures get the same
        // throw treatment as the converter would apply. The closure's
        // success type is inferred from `Ok(#call_expr)` — `_` rather
        // than the source fn's literal return-type tokens, so type
        // aliases (`ZResult`, `MyResult`, ...) used by the source fn
        // need not be in scope at the generated wrappers' include site.
        // `env` is owned by value in the extern signature; pass `&mut env`
        // to the throw fn.
        quote! {
            {
                let __r: ::core::result::Result<_, __JniErr> = (|| {
                    #(#prelude)*
                    Ok(#call_expr)
                })();
                match __r {
                    Ok(__v)  => #conv(&mut env, __v),
                    Err(__e) => {
                        #throw_fn(&mut env, &__e);
                        #on_err
                    }
                }
            }
        }
    } else {
        // Non-throwing converter: returns `Result<wire, __JniErr>`. Should
        // not Err in practice (no throw configured for this return type);
        // sentinel otherwise.
        quote! {
            {
                let __r: ::core::result::Result<#wire_with_lifetime, __JniErr> = (|| {
                    #(#prelude)*
                    let __v = #call_expr;
                    #conv(&mut env, __v)
                })();
                __r.unwrap_or_else(|_| #on_err)
            }
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

/// Last-segment ident of a `TypeKey` — e.g. `"Publisher<'static>"` →
/// `"Publisher"`, `"AdvancedSubscriber<()>"` → `"AdvancedSubscriber"`. Used by
/// the structured builders ([`JniExt::kotlin_class`],
/// [`JniExt::kotlin_value_type`]) to derive a default Kotlin class name from
/// the Rust type-key. Panics for non-path types (e.g. closures, references) —
/// chain `.kotlin_name(...)` to set the name explicitly in that case.
fn rust_short_name(key: &TypeKey) -> String {
    let ty = key.to_type();
    if let syn::Type::Path(tp) = &ty {
        if let Some(last) = tp.path.segments.last() {
            return last.ident.to_string();
        }
    }
    panic!(
        "rust_short_name: cannot derive Kotlin name from type-key `{}` — \
         chain `.kotlin_name(\"X\")` to set it explicitly",
        key.as_str()
    );
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

/// Build the emitted function for an [`JniExt::output_throws`] registration.
///
/// `body` is the user-supplied expression that evaluates to `wire` on the
/// success path (same convention as a plain `output_wrapper` body — errors
/// propagate via `?` for sub-conversion failures, or via `return Err(...)`
/// for custom detection like `i32 < 0`). The framework wraps the body in a
/// `(|| -> Result<wire, __JniErr> { Ok(body) })()` closure to give those
/// `?`/`return` operators somewhere to land, then `match`es the closure's
/// result: on `Ok` it returns the wire value; on `Err` it invokes the
/// generated `throw_<short>` free function (path resolved via
/// [`exception_throw_path`]) and substitutes `sentinel`. The emitted
/// function therefore returns the bare `wire` type — the converter has
/// fully consumed the error case by throwing.
fn build_output_throws_fn(
    rust: &syn::Type,
    wire: &syn::Type,
    body: &syn::Expr,
    throw_fn: &syn::Path,
    sentinel: &TokenStream,
) -> syn::ItemFn {
    let name = output_name(rust, wire);
    let wire_with_lifetime = annotate_jobject_with_lifetime(wire, "a");
    syn::parse_quote!(
        #[allow(non_snake_case, unused_mut, unused_variables, unused_braces, dead_code)]
        pub(crate) unsafe fn #name<'a>(env: &mut jni::JNIEnv<'a>, v: #rust) -> #wire_with_lifetime {
            let __r: ::core::result::Result<#wire_with_lifetime, __JniErr> =
                (|| -> ::core::result::Result<#wire_with_lifetime, __JniErr> { Ok(#body) })();
            match __r {
                Ok(__w) => __w,
                Err(__e) => {
                    // `env` is `&mut JNIEnv` — pass it straight through;
                    // the generated `throw_<short>` fn takes the same
                    // reference type and a `&__e` borrow of the error.
                    #throw_fn(env, &__e);
                    #sentinel
                }
            }
        }
    )
}

/// Sentinel value to return through the wrapper signature when the inner
/// closure errors. Must compile against any wire type we emit.
fn sentinel_for_wire(wire: &syn::Type) -> TokenStream {
    // Unit wire (void-returning wrappers): the value *is* the sentinel.
    if let syn::Type::Tuple(t) = wire {
        if t.elems.is_empty() {
            return quote!(());
        }
    }
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

/// Detect whether an input converter's return type is `_::ZResult<OwnedObject<_>>`
/// (or whatever the `zresult` path happens to be — we only inspect the last
/// segment). Drives the borrow/consume codegen at the call site and the
/// `NativeHandle`-typed parameter detection in the Kotlin wrapper emitter.
pub(crate) fn converter_returns_owned_object(output: &syn::ReturnType) -> bool {
    let syn::ReturnType::Type(_, ty) = output else { return false; };
    let syn::Type::Path(tp) = &**ty else { return false; };
    let Some(last) = tp.path.segments.last() else { return false; };
    let syn::PathArguments::AngleBracketed(args) = &last.arguments else { return false; };
    let Some(syn::GenericArgument::Type(inner)) = args.args.first() else { return false; };
    let syn::Type::Path(itp) = inner else { return false; };
    let Some(last_inner) = itp.path.segments.last() else { return false; };
    last_inner.ident == "OwnedObject"
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
                    .map_err(|e| <__JniErr as ::core::convert::From<String>>::from(format!("decode_string: {}", e)))?;
                s.into()
            }),
        ),
        "Vec < u8 >" => (
            syn::parse_quote!(jni::objects::JByteArray),
            syn::parse_quote!({
                env.convert_byte_array(v)
                    .map_err(|e| <__JniErr as ::core::convert::From<String>>::from(format!("decode_byte_array: {}", e)))?
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
                    .map_err(|e| <__JniErr as ::core::convert::From<String>>::from(format!("encode_string: {}", e)))?
            }),
        ),
        "Vec < u8 >" => (
            syn::parse_quote!(jni::objects::JByteArray),
            syn::parse_quote!({
                env.byte_array_from_slice(v.as_slice())
                    .map_err(|e| <__JniErr as ::core::convert::From<String>>::from(format!("encode_byte_array: {}", e)))?
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
    registry: &Registry<KotlinMeta>,
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
                    .map_err(|e| <__JniErr as ::core::convert::From<String>>::from(format!("Option unbox: {}", e)))?;
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
    registry: &Registry<KotlinMeta>,
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
                    .map_err(|e| <__JniErr as ::core::convert::From<String>>::from(format!("Option box: {}", e)))?
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
    registry: &Registry<KotlinMeta>,
) -> Option<(syn::Type, syn::Expr)> {
    let stem = derive_callback_stem(args);

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
                // The callback's `run` method takes the Kotlin equivalent
                // of this Rust arg type, not the callback interface itself.
                // Look up the registered FQN and slash-encode it for the
                // JVM method descriptor.
                let arg_key = TypeKey::from_type(arg_ty).as_str().to_string();
                let arg_fqn = ext.kotlin_type_fqns
                    .iter()
                    .find(|(k, _)| k == &arg_key)
                    .map(|(_, v)| v.replace('.', "/"))
                    .unwrap_or_else(|| "java/lang/Object".to_string());
                sig.push_str(&format!("L{};", arg_fqn));
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
            .map_err(|e| <__JniErr as ::core::convert::From<String>>::from(format!("Unable to retrieve JVM: {}", e)))?);
        let callback_global_ref = env.new_global_ref(&v)
            .map_err(|e| <__JniErr as ::core::convert::From<String>>::from(format!("Unable to global-ref callback: {}", e)))?;
        Box::new(move |#(#arg_names: #arg_pat_ty),*| {
            let _ = (|| -> ::core::result::Result<(), __JniErr> {
                let mut env = java_vm
                    .attach_current_thread_as_daemon()
                    .map_err(|e| <__JniErr as ::core::convert::From<String>>::from(format!("Attach thread for {}: {}", #stem_lit, e)))?;
                #(#fixed_preludes)*
                env.call_method(
                    &callback_global_ref,
                    "run",
                    #sig_lit,
                    &[#(#jvalue_exprs),*],
                )
                .map_err(|e| {
                    let _ = env.exception_describe();
                    <__JniErr as ::core::convert::From<String>>::from(e.to_string())
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
/// pointer-shape primitives like `Box::into_raw`-as-`jlong`.
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
    registry: &Registry<KotlinMeta>,
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
                        .map_err(|e| <__JniErr as ::core::convert::From<String>>::from(format!(#err_prefix, e)))? as _;
                    let #fname_ident = #field_conv(env, &#raw_ident)?;
                });
            }
            Some((sig, _, true)) => {
                let tmp_ident = format_ident!("__{}_jobj", fname_ident);
                field_preludes.push(quote! {
                    let #tmp_ident: jni::objects::JObject = env.get_field(v, #camel, #sig)
                        .and_then(|val| val.l())
                        .map_err(|e| <__JniErr as ::core::convert::From<String>>::from(format!(#err_prefix, e)))?;
                    let #raw_ident: #field_wire = #tmp_ident.into();
                    let #fname_ident = #field_conv(env, &#raw_ident)?;
                });
            }
            None => {
                // Wire is JObject — fetch via .l() and pass by reference.
                field_preludes.push(quote! {
                    let #raw_ident: jni::objects::JObject = env.get_field(v, #camel, "Ljava/lang/Object;")
                        .and_then(|val| val.l())
                        .map_err(|e| <__JniErr as ::core::convert::From<String>>::from(format!(#err_prefix, e)))?;
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
    registry: &Registry<KotlinMeta>,
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
        .map_err(|e| <__JniErr as ::core::convert::From<String>>::from(format!("encode struct: {}", e)))?;
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

/// Given a source type's wire shape, return the Java class to test via
/// `instanceof` and a prelude that narrows the dispatcher's
/// `v: &jni::objects::JObject` into something the source's existing
/// decoder accepts. The third element is the `decoded_ref` expression
/// passed as the decoder's `v` argument — typically `&__narrowed`,
/// except `JObject` is identity (`v` directly).
///
/// `jlong`-wired sources (opaque handles) **require** a typed FQN in
/// `kotlin_type_fqns`. The generated arm does `instanceof <FQN>` +
/// `peek()` — each opaque source has its own Java class, so multiple
/// opaque sources in one `impl Into<T>` dispatcher are distinguishable.
/// Works for both Borrow (read lock keeps `ptr` valid) and Consume
/// (write lock + null-after-action keeps `ptr` valid during the JNI
/// call). Missing-FQN panics at build time — register a typed FQN
/// (see `JniExt::kotlin_type_fqn`) and ensure the corresponding
/// Kotlin class exists.
///
/// Returns `None` for wires not covered by the table — caller treats it
/// as a hard error (the source type can't participate in
/// `impl Into<T>` dispatch via this generic builder).
fn jobject_to_wire_adapter(
    wire: &syn::Type,
    src_ty: &syn::Type,
    kotlin_type_fqns: &[(String, String)],
) -> Option<(String, TokenStream, TokenStream)> {
    let key = TypeKey::from_type(wire).as_str().to_string();
    match key.as_str() {
        // ── Boxed primitives: unbox via the standard Java accessor ────
        "jni :: sys :: jlong" => {
            let src_key = TypeKey::from_type(src_ty).as_str().to_string();
            let fqn = kotlin_type_fqns
                .iter()
                .find(|(k, _)| k == &src_key)
                .map(|(_, v)| v.replace('.', "/"))
                .unwrap_or_else(|| {
                    panic!(
                        "jobject_to_wire_adapter: opaque source `{}` (jlong wire) has no \
                         typed Kotlin FQN registered. Register one via \
                         `JniExt::kotlin_type_fqn(\"{}\", \"<package>.JNI<Type>\")` and \
                         ensure the corresponding Kotlin class exists.",
                        src_key, src_key
                    )
                });
            Some((
                fqn,
                quote!(
                    let __narrowed: jni::sys::jlong = env
                        .call_method(v, "peek", "()J", &[])
                        .and_then(|val| val.j())
                        .map_err(|e| <__JniErr as ::core::convert::From<String>>::from(format!("NativeHandle.peek: {}", e)))?;
                ),
                quote!(&__narrowed),
            ))
        }
        "jni :: sys :: jint" => Some((
            "java/lang/Integer".to_string(),
            quote!(
                let __narrowed: jni::sys::jint = env
                    .call_method(v, "intValue", "()I", &[])
                    .and_then(|val| val.i())
                    .map_err(|e| <__JniErr as ::core::convert::From<String>>::from(format!("Integer.intValue: {}", e)))?;
            ),
            quote!(&__narrowed),
        )),
        "jni :: sys :: jshort" => Some((
            "java/lang/Short".to_string(),
            quote!(
                let __narrowed: jni::sys::jshort = env
                    .call_method(v, "shortValue", "()S", &[])
                    .and_then(|val| val.s())
                    .map_err(|e| <__JniErr as ::core::convert::From<String>>::from(format!("Short.shortValue: {}", e)))?;
            ),
            quote!(&__narrowed),
        )),
        "jni :: sys :: jbyte" => Some((
            "java/lang/Byte".to_string(),
            quote!(
                let __narrowed: jni::sys::jbyte = env
                    .call_method(v, "byteValue", "()B", &[])
                    .and_then(|val| val.b())
                    .map_err(|e| <__JniErr as ::core::convert::From<String>>::from(format!("Byte.byteValue: {}", e)))?;
            ),
            quote!(&__narrowed),
        )),
        "jni :: sys :: jboolean" => Some((
            "java/lang/Boolean".to_string(),
            quote!(
                let __narrowed: jni::sys::jboolean = env
                    .call_method(v, "booleanValue", "()Z", &[])
                    .and_then(|val| val.z())
                    .map(|b| if b { 1u8 } else { 0u8 })
                    .map_err(|e| <__JniErr as ::core::convert::From<String>>::from(format!("Boolean.booleanValue: {}", e)))?;
            ),
            quote!(&__narrowed),
        )),
        "jni :: sys :: jfloat" => Some((
            "java/lang/Float".to_string(),
            quote!(
                let __narrowed: jni::sys::jfloat = env
                    .call_method(v, "floatValue", "()F", &[])
                    .and_then(|val| val.f())
                    .map_err(|e| <__JniErr as ::core::convert::From<String>>::from(format!("Float.floatValue: {}", e)))?;
            ),
            quote!(&__narrowed),
        )),
        "jni :: sys :: jdouble" => Some((
            "java/lang/Double".to_string(),
            quote!(
                let __narrowed: jni::sys::jdouble = env
                    .call_method(v, "doubleValue", "()D", &[])
                    .and_then(|val| val.d())
                    .map_err(|e| <__JniErr as ::core::convert::From<String>>::from(format!("Double.doubleValue: {}", e)))?;
            ),
            quote!(&__narrowed),
        )),
        // ── Reference wrappers — wrap `v.as_raw()`, release after use ─
        "jni :: objects :: JString" => Some((
            "java/lang/String".to_string(),
            quote!(
                let __narrowed: jni::objects::JString =
                    unsafe { jni::objects::JString::from_raw(v.as_raw()) };
            ),
            quote!(&__narrowed),
        )),
        "jni :: objects :: JByteArray" => Some((
            "[B".to_string(),
            quote!(
                let __narrowed: jni::objects::JByteArray =
                    unsafe { jni::objects::JByteArray::from_raw(v.as_raw()) };
            ),
            quote!(&__narrowed),
        )),
        // ── JObject ───────────────────────────────────────────────────
        "jni :: objects :: JObject" | "jni :: sys :: jobject" => {
            // Need an explicit Java class — pull from kotlin_type_fqns.
            let src_key = TypeKey::from_type(src_ty).as_str().to_string();
            let fqn = kotlin_type_fqns
                .iter()
                .find(|(k, _)| k == &src_key)
                .map(|(_, v)| v.replace('.', "/"))?;
            Some((fqn, quote!(), quote!(v)))
        }
        _ => None,
    }
}

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

/// `OwnedObject<T>` definition emitted into the destination Rust file.
///
/// A non-owning borrow wrapper around a `*const T` whose backing
/// `Box<T>` lives on the Java side. The Java side hands Rust the
/// pointer under its `NativeHandle.withPtr` read lock; for the
/// duration of the JNI call the heap allocation is guaranteed live,
/// so `Deref<Target = T>` exposing `&*ptr` is sound. The wrapper has
/// no `Drop`: nothing is freed here, the Box stays with Java.
///
/// By-value `T` extraction is intentionally NOT through this wrapper.
/// Consume call sites use `*Box::from_raw(ptr)` inline, taking
/// ownership of Java's slot; `NativeHandle.consume` (write-lock +
/// atomic null) sequences that against any concurrent borrow.
///
/// Co-locating the definition with the converters keeps the generated
/// file self-contained — no `use` statement or runtime-support module
/// is required from the host crate.
pub(crate) fn owned_object_prerequisite_items() -> Vec<syn::Item> {
    vec![
        syn::parse_quote!(
            /// See module-level docs at [`owned_object_prerequisite_items`].
            #[allow(dead_code)]
            pub(crate) struct OwnedObject<T: ?Sized> {
                ptr: *const T,
            }
        ),
        syn::parse_quote!(
            impl<T: ?Sized> std::ops::Deref for OwnedObject<T> {
                type Target = T;
                fn deref(&self) -> &Self::Target {
                    unsafe { &*self.ptr }
                }
            }
        ),
        syn::parse_quote!(
            impl<T: ?Sized> OwnedObject<T> {
                /// Borrow a `T` whose backing `Box<T>` lives on the
                /// Java side. Stores only the pointer; the wrapper
                /// does not own the heap allocation and never frees
                /// it on drop.
                ///
                /// # Safety
                ///
                /// `ptr` must be the result of an earlier
                /// `Box::into_raw(Box::new(v))` and the allocation
                /// must still be live (Java still owns it). The Java
                /// side is responsible for sequencing this call
                /// against any concurrent free or consume (via
                /// `NativeHandle.withPtr` read-lock vs `consume` /
                /// `close` write-lock) so the borrow cannot race a
                /// deallocation on the same pointer.
                #[allow(dead_code)]
                pub(crate) unsafe fn from_raw(ptr: *const T) -> Self {
                    Self { ptr }
                }
            }
        ),
    ]
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
    fn entry(wire: syn::Type, conv_name: &str, niches: Niches) -> TypeEntry<KotlinMeta> {
        let ident = syn::Ident::new(conv_name, proc_macro2::Span::call_site());
        let func: syn::ItemFn = syn::parse_quote!(
            unsafe fn #ident<'env, 'v>(
                env: &mut jni::JNIEnv<'env>,
                v: &#wire,
            ) -> ::core::result::Result<(), __JniErr> {
                Ok(())
            }
        );
        TypeEntry {
            destination: wire,
            function: func,
            subs: vec![],
            required: false,
            niches,
            into_sources: None,
            metadata: KotlinMeta::default(),
        }
    }

    fn install_input(reg: &mut Registry<KotlinMeta>, ty_str: &str, rank: usize, e: TypeEntry<KotlinMeta>) {
        reg.input_types[rank].insert(TypeKey::parse(ty_str), Some(e));
    }
    fn install_output(reg: &mut Registry<KotlinMeta>, ty_str: &str, rank: usize, e: TypeEntry<KotlinMeta>) {
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
