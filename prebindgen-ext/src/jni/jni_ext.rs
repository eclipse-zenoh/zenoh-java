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
//!   alongside the registered throwable-class entries (`.throwable()`).
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
use crate::core::prebindgen_ext::{ConverterImpl, IntoSource, IntoSourceMode, PrebindgenExt, Stage};
use crate::core::registry::{extract_fn_trait_args, Registry, TypeKey};
use crate::jni::wire_access::jni_field_access;
use crate::util::snake_to_camel;

// ──────────────────────────────────────────────────────────────────────
// Language metadata (PrebindgenExt::Metadata for JniExt)
// ──────────────────────────────────────────────────────────────────────

/// Folded nullability / collection layers wrapping a closeable native
/// handle, outermost first. Mirrors how the type folds: the opaque-handle
/// leaf is [`CloseStrategy::Direct`]; an `Option<_>` wrapper adds
/// [`CloseStrategy::Nullable`]; a collection wrapper would add
/// [`CloseStrategy::Iterable`]. Drives both the typed Kotlin rendering of
/// a handle-bearing field/return and the generated `close()` expression,
/// uniformly across whatever wrappers compose.
#[derive(Clone, Debug)]
pub enum CloseStrategy {
    /// The receiver *is* the handle.
    Direct,
    /// `T?` — receiver may be null.
    Nullable(Box<CloseStrategy>),
    /// `List<T>` — receiver is a collection. EXTENSION POINT: no
    /// `Vec<Handle>` shape exists today, so the emitters guard this arm
    /// loudly rather than silently mis-generating.
    Iterable(Box<CloseStrategy>),
}

/// Folded description of a closeable native handle reached through zero or
/// more wrapper layers. Set at the opaque-handle leaf, transformed by each
/// wrapper as the type folds (see [`CloseStrategy`]), and read by every
/// typed-surface emitter (data-class fields, struct encode/decode,
/// `classify_return`, param classification) so "is this an owned closeable
/// handle, what is its Kotlin class, how do I close it" has one source of
/// truth instead of a parallel ad-hoc decision tree.
#[derive(Clone, Debug)]
pub struct HandleInfo {
    /// Canonical [`TypeKey`] string of the leaf opaque handle (e.g.
    /// `"ZKeyExpr"`); look up [`JniExt::kotlin_type_fqns`] for the typed
    /// Kotlin FQN.
    pub leaf_key: String,
    /// `false` for `&T` borrows — still an opaque handle (param
    /// classification needs this), but not the holder's to close, so
    /// `close()` emission skips it.
    pub owned: bool,
    /// Nullability / collection layers.
    pub strategy: CloseStrategy,
}

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
    /// by [`JniExt::throws`]; the Kotlin emitter uses this for
    /// `@Throws` annotations on the corresponding wrappers. `None` means
    /// "non-throwing converter" (no `@Throws` emitted).
    pub throws: Option<String>,
    /// Rust path of the generated `throw_<RustShortName>` free function
    /// the framework invokes as `<throws_action>(&mut env, &err)` for
    /// wrapper-internal failures (e.g. input-decode `?` propagation) that
    /// surface above this converter. Populated alongside
    /// [`Self::throws`] by [`JniExt::throws`]; `None` when no
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
    /// Populated by [`JniExt::throws`] with `args[0]`'s canonical
    /// key for arity-1 wrappers, and inherited by the built-in
    /// `Option<_>` / `Vec<_>` / `&_` rank-1 handlers from their inner
    /// type's metadata. `None` for plain values and arity-0 converters.
    pub value_rust_key: Option<String>,
    /// Present iff this (possibly wrapped) value is an opaque native
    /// handle. Set at the opaque-handle leaf and folded outward by the
    /// rank-1 `&_` / `Option<_>` handlers and the `lookup_*` composed
    /// branches. The single source of truth for typed-handle rendering
    /// and `close()` generation — see [`HandleInfo`].
    pub handle: Option<HandleInfo>,
}

impl KotlinMeta {
    pub fn from_name(name: impl Into<String>) -> Self {
        Self {
            kotlin_name: Some(name.into()),
            throws: None,
            throws_action: None,
            value_rust_key: None,
            handle: None,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────
// Structured type-conversion configuration
// ──────────────────────────────────────────────────────────────────────

/// Per-exception-class configuration (driven by
/// [`JniExt::throwable`]).
///
/// One entry per Rust error type the binding surfaces to the JVM as a
/// Java exception. Declaration order matters: `exceptions[0]` is the
/// *primary* — its `From<String>` impl is the universal converter-failure
/// path and its Kotlin FQN is used for `NativeHandle`'s closed-handle
/// exception. The throw function emitted into the generated file
/// (`throw_<rust_short>`) does the `find_class`/`throw_new` dance and
/// is referenced by the throws-marked wrapper code through
/// [`KotlinMeta::throws_action`].
#[derive(Clone)]
pub(crate) struct ExceptionConfig {
    /// Absolute Rust path of the error type, as a `syn::Type::Path`
    /// (e.g. `zenoh_flat::errors::ZError`). Used both to splice the
    /// `pub(crate) type __JniErr = ...` alias and as the function-
    /// argument type of the generated `throw_<short>`. Stored as a
    /// `syn::Type` so it round-trips identically with the
    /// closure-slot exception bindings in [`WrapperFn`] — both ends
    /// spell the type the same way.
    pub rust_type: syn::Type,
    /// Last path segment of `rust_type` (e.g. `"ZError"`). Used to
    /// derive the `throw_<short>` function name and to provide the
    /// Kotlin class name (relative; qualified against
    /// [`JniExt::package`]). Exception class names are not currently
    /// routed through any `kotlin_*_name_mangle` closure — the
    /// short-name lands in the FQN verbatim.
    pub rust_short: String,
    /// Kotlin fully-qualified exception class name (e.g.
    /// `"io.zenoh.jni.ZError"`) — `<package>.<rust_short>`. Used for the
    /// Kotlin class file path, `@Throws` annotations, and the JNI
    /// `find_class("io/zenoh/jni/ZError")` literal inside the generated
    /// `throw_<short>` body.
    pub kotlin_fqn: String,
    /// Identifier of the generated `throw_<short>` function.
    pub throw_fn_name: syn::Ident,
}

/// Per-opaque-handle configuration (driven by `JniExt::kotlin_ptr_class`).
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
}

/// Per-enum configuration (driven by `JniExt::kotlin_enum`).
///
/// Marks a `#[prebindgen]`-scanned `enum` as a Kotlin enum class — the
/// rank-0 converter arms emit `jint ↔ Rust enum` bodies (via `TryFrom<i32>`
/// for input and `as jni::sys::jint` for output), and the Kotlin emitter
/// writes an `enum class` file with SCREAMING_SNAKE_CASE variants and a
/// discriminant-keyed `fromInt(...)` companion. The Kotlin FQN lives in
/// the surrounding [`TypeConfig::kotlin_name`] slot, same as
/// [`OpaqueConfig`].
#[derive(Clone, Default)]
pub(crate) struct EnumConfig {
    /// When `false` (default), the unified Kotlin emitter writes an
    /// `enum class` `.kt` file for this enum. Set to `true` by
    /// [`JniExt::suppress_kotlin_code`] when the Kotlin source is
    /// hand-maintained — only the Rust-side converter wires up.
    pub suppress_kotlin_code: bool,
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
    /// If `Some`, this is a `#[prebindgen]` enum mirrored as a Kotlin
    /// `enum class` — gets jint wire (input + output via `TryFrom<i32>`
    /// / `as jint`) and a generated `.kt` file. Mutually exclusive with
    /// [`Self::opaque`]; builder enforces it.
    pub enum_cfg: Option<EnumConfig>,
    /// Kotlin FQN override for `impl Fn(...)` keys (the
    /// closure-mangled callback name, e.g. zenoh-jni stamps `JNIOn...`
    /// here via [`JniExt::auto_callback_fqn`]).
    pub callback_kotlin_fqn: Option<String>,
    /// `#[prebindgen]` fn idents promoted onto the generated Kotlin
    /// class for this type via [`JniExt::method`]. For ptr classes,
    /// wrappers become instance methods when the promoted opaque param
    /// matches; for enums/data classes they are emitted in
    /// `companion object`.
    pub methods: Vec<String>,
    /// Set by [`JniExt::throwable`]: the emitted Kotlin class extends
    /// `Exception` and a structured `throw_<short>` is generated that
    /// constructs the JVM object via this type's own output converter.
    /// `Result<_, ThisType>` Err arms route through that throw fn.
    pub throwable: bool,
}

/// Methods promoted to the synthetic package-level wrapper object.
#[derive(Clone, Default)]
pub(crate) struct PackageConfig {
    /// Kotlin subpackage used for package-level wrappers.
    pub subpackage: Option<String>,
    /// `#[prebindgen]` fn idents promoted onto the generated package
    /// object via [`JniExt::method`].
    pub methods: Vec<String>,
}

/// Boxed closure that builds a converter when applied to the wildcard
/// substitutions. Returns `None` to defer (an inner converter the
/// builder depends on isn't yet resolved; the resolver retries on the
/// next phase), or `Some((ty, exc, body))` where:
///
/// * `ty` — the type the body produces. Auto-classified at lookup:
///   a wire shape (or the self-converter case) ⇒ terminal converter
///   with `destination = ty`; a rust type with its own converter ⇒
///   composed as a value-inspecting stage onto that converter's chain.
/// * `exc` — the bound exception **as a Rust type**, matched by exact
///   canonical-form equality against a [`JniExt::throwable`]
///   registration's `rust_type` (use the same full path the
///   registration was declared with, e.g.
///   `parse_quote!(zenoh_flat::errors::ZError)` — no short-name
///   matching). `Some(...)` ⇒ throwing: the body evaluates to
///   `Result<ty, exc>` and is emitted as-is. `None` ⇒ non-throwing:
///   the body evaluates to a bare `ty` and the framework wraps it
///   `Ok(body)` with `Result<ty, __JniErr>` (= `JniBindingError`).
/// * `body` — the closure body. The decision between Ok-wrap vs
///   verbatim is keyed on `exc` (see [`JniExt::build_input_fn`] /
///   [`JniExt::build_output_fn`]).
///
/// Receives `&Registry<KotlinMeta>` so the closure can look up
/// inner-type entries (`registry.output_entry(t)`).
pub(crate) type WrapperFn = Arc<
    dyn Fn(&[syn::Type], &Registry<KotlinMeta>)
            -> Option<(syn::Type, Option<syn::Type>, syn::Expr)>
        + Send
        + Sync,
>;

/// Closure that transforms a Kotlin short name. Installed via the
/// per-kind setters ([`JniExt::kotlin_fun_name_mangle`],
    /// [`JniExt::kotlin_data_class_name_mangle`], etc.); the framework calls
/// the matching closure wherever it needs to derive a Kotlin/JNI
/// short name for a generated element. Closure-unset = identity.
pub(crate) type NameMangle = Arc<dyn Fn(&str) -> String + Send + Sync>;

/// Trait selecting the arity-appropriate impl of
/// [`JniExt::input_wrapper`] / [`JniExt::output_wrapper`]. The phantom
/// type parameter discriminates closures of arity 0..3 so a single
/// public method name accepts any of them. Closures take the wildcard
/// substitutions plus the registry, and return `Some((ty, exc, body))`
/// or `None` (defer to a later resolver phase). See [`WrapperFn`] for
/// the triple's semantics.
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
    F: Fn(&Registry<KotlinMeta>) -> Option<(syn::Type, Option<syn::Type>, syn::Expr)>
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
    F: Fn(&syn::Type, &Registry<KotlinMeta>) -> Option<(syn::Type, Option<syn::Type>, syn::Expr)>
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
    F: Fn(&syn::Type, &syn::Type, &Registry<KotlinMeta>) -> Option<(syn::Type, Option<syn::Type>, syn::Expr)>
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
            -> Option<(syn::Type, Option<syn::Type>, syn::Expr)>
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
    /// (`exceptions[0]`) is the framework `JniBindingError` — emitted as
    /// the `__JniErr` alias by [`Self::prerequisites`] and used for
    /// `NativeHandle`'s closed-handle exception. Populated by repeated
    /// [`Self::throwable`] calls; consumed by:
    /// [`Self::prerequisites`] (framework error type → `__JniErr`),
    /// [`Self::write_exceptions_rust`] (one `throw_<short>` per entry),
    /// [`Self::write_native_handle`] (framework FQN), and
    /// [`Self::lookup_input`] / [`Self::lookup_output`] (per-converter
    /// bound-exception FQN + throw fn).
    pub(crate) exceptions: Vec<ExceptionConfig>,
    /// Single source of truth for the JVM/Kotlin namespace this binding
    /// targets, dot-separated (e.g. `io.zenoh.jni`). Empty = no prefix.
    /// Drives every derived form: slash-separated for `FindClass`,
    /// `_`-mangled for JNI extern idents, and dot-separated for Kotlin
    /// `package` declarations.
    pub package: String,
    /// Sub-package leaf appended to [`Self::package`] for the auto-emitted
    /// callback fun-interface files. Combined as
    /// `<package>.<callback_subpackage>`; empty = same package as
    /// [`Self::package`].
    pub callback_subpackage: String,
    /// Derived: `package.replace('.', '/')`. Read by
    /// [`struct_output_body`] when building `FindClass` strings.
    pub(crate) java_class_prefix: String,
    /// Derived: `"Java_" + package.replace('.', '_') + "_" +
    /// mangle_harness("Native")`. Read by [`mangle_jni_name`] when
    /// building the JNI extern symbol path for every emitted wrapper.
    pub(crate) jni_class_path: String,
    /// Derived: `package + "." + callback_subpackage` (or just `package`
    /// when the subpackage is empty). Also drives the on-disk subdirectory
    /// under the `kotlin_root` passed to [`Self::write_kotlin`]
    /// (`a.b.c` → `a/b/c/`).
    pub(crate) kotlin_callback_package: String,

    /// Mangler for function names (scanned `#[prebindgen]` free fns and
    /// the synthetic `freePtr` destructor). Default = identity; in
    /// zenoh-jni the closure returns `format!("{name}ViaJNI")` so the
    /// generated JNI extern symbols and matching Kotlin `external fun`s
    /// both pick up the `ViaJNI` suffix.
    pub(crate) kotlin_fun_name_mangle: Option<NameMangle>,
    /// Mangler for Kotlin ptr-class names declared via
    /// [`Self::kotlin_ptr_class`]. Default = identity.
    pub(crate) kotlin_ptr_class_name_mangle: Option<NameMangle>,
    /// Mangler for Kotlin data-class names declared via
    /// [`Self::kotlin_data_class`]. Default = identity.
    pub(crate) kotlin_data_class_name_mangle: Option<NameMangle>,
    /// Mangler for [`Self::kotlin_enum`]-declared C-like enum class
    /// names. Default = identity.
    pub(crate) kotlin_enum_name_mangle: Option<NameMangle>,
    /// Mangler for the package-level wrapper object created by
    /// [`Self::kotlin_package`]. Default = identity.
    pub(crate) kotlin_package_name_mangle: Option<NameMangle>,
    /// Mangler for `impl Fn(...)` callback Kotlin class names. The
    /// closure receives the auto-derived callback name
    /// ([`crate::jni::jni_kotlin_ext::derive_callback_name`], always
    /// concatenated parameter type shorts + `"Callback"` suffix — e.g.
    /// `"QueryCallback"`, `"ReplyCallback"`, `"Callback"` for `Fn()`);
    /// the return value is qualified against
    /// [`Self::kotlin_callback_package`]. Default = identity.
    pub(crate) kotlin_callback_name_mangle: Option<NameMangle>,
    /// Mangler for rank-0 user-registered
    /// [`Self::input_wrapper`] / [`Self::output_wrapper`] pattern names
    /// — the Rust short name of the pattern (`Encoding`,
    /// `SetIntersectionLevel`, …). Rank-N wrappers (`Option<_>`,
    /// `ZResult<_>`, `Vec<_>`, `&_`) are NOT routed through any
    /// mangler — they inherit from the inner type's metadata via the
    /// existing rank-N handlers. Default = identity.
    pub(crate) kotlin_wrapper_name_mangle: Option<NameMangle>,
    /// Mangler for the framework "harness" class name —
    /// `"Native"` (the centralized JNI extern holder). Default when
    /// unset = prepend `"JNI"`, so you get `JNINative`. Override to
    /// plug in a different convention.
    pub(crate) kotlin_harness_name_mangle: Option<NameMangle>,
    /// Derived `<rust-type-canonical-string> → <kotlin FQN>` view —
    /// populated alongside [`Self::types`] by the structured builders
    /// ([`Self::kotlin_ptr_class`], [`Self::kotlin_data_class`],
    /// [`Self::callback_input`], [`Self::input_wrapper`] /
    /// [`Self::output_wrapper`]). Internal readers
    /// (`emit_into_dispatcher`, callback FQN merging) consume this flat
    /// list directly; the structured `types` map is the source of
    /// truth.
    pub(crate) kotlin_type_fqns: Vec<(String, String)>,

    /// Structured per-type configuration keyed by canonical Rust type.
    /// One entry per `Rust type ↔ JNI/Kotlin` rule; populated by the
    /// structured builders (`kotlin_ptr_class`, `kotlin_enum`,
    /// `kotlin_data_class`, `input_wrapper`, `output_wrapper`,
    /// `callback_input`). Holds opaque-handle config, enum config,
    /// Kotlin names, and callback FQNs; the converter bodies themselves
    /// live in [`Self::input_wrappers`] / [`Self::output_wrappers`].
    /// The rank-0 dispatch order is opaque → enum → wrapper-table →
    /// primitive → struct.
    pub(crate) types: HashMap<TypeKey, TypeConfig>,

    /// Package-level promoted methods written into a separate wrapper object.
    pub(crate) package_methods: PackageConfig,

    /// `impl Into<target> + Send + 'static` source arms per target type.
    pub(crate) into_sources_map: HashMap<TypeKey, Vec<IntoSource>>,

    /// Per-rank input converters — index `n` holds rank-`n` registrations
    /// keyed by the pattern's `TypeKey`. Rank 0 is non-wildcard (e.g.
    /// `"i32"`); ranks 1..3 carry that many `_` slots (e.g. `"Vec < _ >"`).
    /// Each [`WrapperFn`] closure carries the builder body AND the bound
    /// exception (the closure returns `(ty, exc, body)`); terminal vs
    /// composed is derived at lookup time, throwing vs non-throwing
    /// from the closure's `Option<String>` middle slot.
    pub(crate) input_wrappers: [HashMap<TypeKey, WrapperFn>; 4],

    /// Per-rank output converters. Same shape as [`Self::input_wrappers`].
    pub(crate) output_wrappers: [HashMap<TypeKey, WrapperFn>; 4],

    /// Tracks the last [`Self::kotlin_ptr_class`] key registered so
    /// [`Self::method`] / [`Self::suppress_kotlin_code`] know which
    /// entry to extend. Cleared after each unrelated builder call.
    last_opaque_key: Option<TypeKey>,

    /// Tracks the last rank-0 wrapper registration so chained per-type
    /// builders ([`Self::suppress_kotlin_code`], [`Self::with_kotlin_type`])
    /// know which entry to extend. Set by `input_wrapper` /
    /// `output_wrapper` (rank 0 only), `kotlin_enum`, `callback_input`,
    /// and `kotlin_data_class`; cleared by other unrelated builders.
    last_meta_key: Option<TypeKey>,

    /// Tracks whether the last fluent builder was [`Self::kotlin_package`].
    last_package_target: bool,
}

impl JniExt {
    /// Convenience constructor with sensible defaults; the paths still need
    /// to be set explicitly via the field-mutation builder methods.
    ///
    /// Pre-registers the framework's [`crate::jni::JniBindingError`] as
    /// `exceptions[0]` so it's always available as `throw_JniBindingError`
    /// in the generated bindings and as the default `__JniErr` alias.
    /// Its Kotlin FQN is `(empty).JniBindingError` until [`Self::package`]
    /// is called, then auto-rebases via [`Self::recompute_derived`].
    pub fn new() -> Self {
        let framework_exc = build_exception_config(
            syn::parse_quote!(::prebindgen_ext::jni::JniBindingError),
            "",
            &[],
        );
        let base = Self {
            source_module: syn::parse_str("crate").unwrap(),
            // exceptions[0] is the framework slot (JniBindingError);
            // user `.throwable()` calls append at 1+.
            exceptions: vec![framework_exc],
            package: String::new(),
            callback_subpackage: "callbacks".to_string(),
            java_class_prefix: String::new(),
            jni_class_path: "Java_JNINative".to_string(),
            kotlin_callback_package: "callbacks".to_string(),
            kotlin_fun_name_mangle: None,
            kotlin_ptr_class_name_mangle: None,
            kotlin_data_class_name_mangle: None,
            kotlin_enum_name_mangle: None,
            kotlin_package_name_mangle: None,
            kotlin_callback_name_mangle: None,
            kotlin_wrapper_name_mangle: None,
            kotlin_harness_name_mangle: None,
            kotlin_type_fqns: Vec::new(),
            types: HashMap::new(),
            package_methods: PackageConfig::default(),
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
            last_package_target: false,
        };
        // Built-in rank-2 `Result<_, _>` peel: every Result<T, E> succeeds
        // as T and throws E on Err. E must be declared throwable via
        // `.throwable()` (chained after a class declaration); the
        // resulting peel stage is composed via `lookup_output`'s
        // exact-canonical-form match in `find_exception`. Consumers may
        // override per-binding by registering a more specific rank-1
        // `Result<_, ConcreteErr>` (rank-1 phase fires before rank-2).
        base.output_wrapper(
            syn::parse_quote!(Result<_, _>),
            |ok: &syn::Type, err: &syn::Type, _: &Registry<KotlinMeta>| {
                Some((ok.clone(), Some(err.clone()), syn::parse_quote!(v)))
            },
        )
    }
    pub fn source_module(mut self, p: syn::Path) -> Self {
        self.source_module = p;
        self
    }

    /// Mark the most recently declared class
    /// ([`Self::kotlin_data_class`], [`Self::kotlin_ptr_class`], or
    /// [`Self::kotlin_enum`]) as throwable. Two effects:
    ///
    /// 1. The emitted Kotlin class extends `Exception` — see
    ///    [`crate::jni::jni_kotlin_ext`] for the renderer's branch.
    /// 2. A `throw_<RustShortName>` free function is generated that
    ///    constructs the JVM object via this type's own output converter
    ///    and throws it. The Result-peel stages built by the rank-2
    ///    `Result<_, _>` wrapper (`JniExt::new`) call into this fn.
    ///
    /// Chains exactly like [`Self::method`] / [`Self::suppress_kotlin_code`];
    /// panics if no class was just declared. The framework's own
    /// [`crate::jni::JniBindingError`] is pre-registered at
    /// `exceptions[0]` directly inside [`Self::new`] and bypasses this
    /// builder, so its stub-template Kotlin emission stays as-is.
    pub fn throwable(mut self) -> Self {
        let key = self.last_meta_key.clone().expect(
            "JniExt::throwable must be chained immediately after a \
             `kotlin_data_class`, `kotlin_ptr_class`, or `kotlin_enum` call",
        );
        let rust_type = key.to_type();
        let cfg = build_exception_config(rust_type, &self.package, &self.exceptions);
        self.exceptions.push(cfg);
        let entry = self.types.get_mut(&key).expect("type entry vanished");
        entry.throwable = true;
        self
    }

    /// Set the JVM/Kotlin base package (dot-separated, e.g.
    /// `"io.zenoh.jni"`). All derived forms (`java_class_prefix`,
    /// `kotlin_callback_package`) are recomputed.
    pub fn kotlin_package_prefix(mut self, p: impl Into<String>) -> Self {
        self.package = p.into().trim_matches('.').trim_matches('/').to_string();
        self.recompute_derived();
        self
    }
    /// Set the closure that mangles the framework "harness" class name
    /// `"Native"` (the centralized extern holder). Default = prepend
    /// `"JNI"` (yielding `JNINative`). Affects the generated Kotlin
    /// class name and, via [`Self::jni_class_path`], the JNI extern
    /// symbol path on the Rust side.
    pub fn kotlin_harness_name_mangle<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> String + Send + Sync + 'static,
    {
        self.kotlin_harness_name_mangle = Some(Arc::new(f));
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
    /// Set the closure that mangles function names. Called for every
    /// scanned `#[prebindgen]` free function and the synthetic
    /// `freePtr` destructor; receives the camelCased Kotlin-side name
    /// and returns the final form (e.g. `"putPublisher"` →
    /// `"putPublisherViaJNI"`). Default = identity.
    pub fn kotlin_fun_name_mangle<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> String + Send + Sync + 'static,
    {
        self.kotlin_fun_name_mangle = Some(Arc::new(f));
        self
    }
    /// Set the closure that mangles Kotlin ptr-class names declared
    /// via [`Self::kotlin_ptr_class`]. Receives the Rust short name.
    /// Default = identity.
    pub fn kotlin_ptr_class_name_mangle<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> String + Send + Sync + 'static,
    {
        self.kotlin_ptr_class_name_mangle = Some(Arc::new(f));
        self
    }
    /// Set the closure that mangles Kotlin data-class names declared
    /// via [`Self::kotlin_data_class`]. Receives the Rust short name.
    /// Default = identity.
    pub fn kotlin_data_class_name_mangle<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> String + Send + Sync + 'static,
    {
        self.kotlin_data_class_name_mangle = Some(Arc::new(f));
        self
    }
    /// Set the closure that mangles [`Self::kotlin_enum`]-declared
    /// enum class names. Receives the Rust short name. Default =
    /// identity.
    pub fn kotlin_enum_name_mangle<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> String + Send + Sync + 'static,
    {
        self.kotlin_enum_name_mangle = Some(Arc::new(f));
        self
    }
    /// Set the closure that mangles the package-level wrapper object
    /// name created by [`Self::kotlin_package`]. Default = identity.
    pub fn kotlin_package_name_mangle<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> String + Send + Sync + 'static,
    {
        self.kotlin_package_name_mangle = Some(Arc::new(f));
        self
    }
    /// Set the closure that mangles `impl Fn(...)` callback class
    /// names. Receives the auto-derived callback name
    /// ([`crate::jni::jni_kotlin_ext::derive_callback_name`], always
    /// concatenated parameter type shorts + `"Callback"` suffix — e.g.
    /// `"QueryCallback"`, `"ReplyCallback"`, `"Callback"` for `Fn()`);
    /// the returned relative name is qualified against
    /// [`Self::kotlin_callback_package`]. Default = identity.
    pub fn kotlin_callback_name_mangle<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> String + Send + Sync + 'static,
    {
        self.kotlin_callback_name_mangle = Some(Arc::new(f));
        self
    }
    /// Set the closure that mangles rank-0
    /// [`Self::input_wrapper`] / [`Self::output_wrapper`] pattern
    /// names (e.g. `"Encoding"`). Rank-N patterns are NOT routed
    /// through this closure — they inherit from the inner type's
    /// metadata via the existing rank-N handlers, preserving the
    /// structural invariant `Option<Encoding>` ↔ `JNIEncoding?`.
    /// Default = identity.
    pub fn kotlin_wrapper_name_mangle<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> String + Send + Sync + 'static,
    {
        self.kotlin_wrapper_name_mangle = Some(Arc::new(f));
        self
    }

    /// Declare a package-level wrapper object under a subpackage.
    /// Subsequent [`Self::method`] calls attach to this synthetic target
    /// instead of the orphaned object.
    pub fn kotlin_package(mut self, subpackage: impl Into<String>) -> Self {
        self.last_opaque_key = None;
        self.last_meta_key = None;
        self.package_methods.subpackage = Some(subpackage.into().trim_matches('.').to_string());
        self.last_package_target = true;
        self
    }

    /// Recompute the derived caches (`java_class_prefix`,
    /// `jni_class_path`, `kotlin_callback_package`) from (`package`,
    /// `kotlin_harness_name_mangle`, `callback_subpackage`). Called by
    /// every setter that touches one of those source fields. The JNI
    /// extern symbol path resolves to the centralized Native object,
    /// whose mangled name comes from the harness mangle (default
    /// `"JNI" + n` → `JNINative`).
    fn recompute_derived(&mut self) {
        self.java_class_prefix = self.package.replace(".", "/");
        let native_class = self.mangle_harness("Native");
        self.jni_class_path = if self.package.is_empty() {
            format!("Java_{}", native_class)
        } else {
            format!("Java_{}_{}", self.package.replace(".", "_"), native_class)
        };
        self.kotlin_callback_package = if self.package.is_empty() {
            self.callback_subpackage.clone()
        } else if self.callback_subpackage.is_empty() {
            self.package.clone()
        } else {
            format!("{}.{}", self.package, self.callback_subpackage)
        };
        // Re-anchor every exception's Kotlin FQN against the (possibly
        // new) package. Each entry's `rust_short` is stable; the FQN is
        // a derived view. In practice `package` is called first in
        // every binding's build script, before any exception class is
        // declared, so the framework slot at index 0 always re-derives
        // cleanly.
        for exc in &mut self.exceptions {
            exc.kotlin_fqn = if self.package.is_empty() {
                exc.rust_short.clone()
            } else {
                format!("{}.{}", self.package, exc.rust_short)
            };
        }
    }

    /// Apply [`Self::kotlin_fun_name_mangle`] to `name`, returning the
    /// closure result or `name` verbatim when unset. Called everywhere
    /// the framework derives a function-shaped Kotlin/JNI short name —
    /// scanned `#[prebindgen]` extern symbols, the synthetic `freePtr`
    /// destructor, and the Kotlin-side `external fun` that pairs with
    /// each.
    pub(crate) fn mangle_fun(&self, name: &str) -> String {
        match &self.kotlin_fun_name_mangle {
            Some(f) => f(name),
            None => name.to_string(),
        }
    }
    /// Apply [`Self::kotlin_ptr_class_name_mangle`] to `name`,
    /// returning the closure result or `name` verbatim when unset.
    pub(crate) fn mangle_ptr_class(&self, name: &str) -> String {
        match &self.kotlin_ptr_class_name_mangle {
            Some(f) => f(name),
            None => name.to_string(),
        }
    }
    /// Apply [`Self::kotlin_data_class_name_mangle`] to `name`,
    /// returning the closure result or `name` verbatim when unset.
    pub(crate) fn mangle_data_class(&self, name: &str) -> String {
        match &self.kotlin_data_class_name_mangle {
            Some(f) => f(name),
            None => name.to_string(),
        }
    }
    /// Apply [`Self::kotlin_enum_name_mangle`] to `name`, returning the
    /// closure result or `name` verbatim when unset.
    pub(crate) fn mangle_enum(&self, name: &str) -> String {
        match &self.kotlin_enum_name_mangle {
            Some(f) => f(name),
            None => name.to_string(),
        }
    }
    /// Apply [`Self::kotlin_callback_name_mangle`] to `name`, returning
    /// the closure result or `name` verbatim when unset.
    pub(crate) fn mangle_callback(&self, name: &str) -> String {
        match &self.kotlin_callback_name_mangle {
            Some(f) => f(name),
            None => name.to_string(),
        }
    }
    /// Apply [`Self::kotlin_wrapper_name_mangle`] to `name`, returning
    /// the closure result or `name` verbatim when unset.
    pub(crate) fn mangle_wrapper(&self, name: &str) -> String {
        match &self.kotlin_wrapper_name_mangle {
            Some(f) => f(name),
            None => name.to_string(),
        }
    }
    /// Apply [`Self::kotlin_harness_name_mangle`] to `name`. The
    /// closure defaults to `|n| format!("JNI{n}")` when unset, so calling
    /// `mangle_harness("Native")` yields `"JNINative"`.
    pub(crate) fn mangle_harness(&self, name: &str) -> String {
        match &self.kotlin_harness_name_mangle {
            Some(f) => f(name),
            None => format!("JNI{name}"),
        }
    }
    /// The mangled name of the centralized Native object that hosts
    /// every JNI `external fun`. Drives both the Kotlin class emission
    /// and the JNI extern symbol path on the Rust side.
    pub(crate) fn jni_native_class_name(&self) -> String {
        self.mangle_harness("Native")
    }
    /// The mangled name of the package-level wrapper object used by
    /// [`Self::kotlin_package`].
    pub(crate) fn jni_package_class_name(&self) -> String {
        let name = self
            .package_methods
            .subpackage
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("Package");
        match &self.kotlin_package_name_mangle {
            Some(f) => f(name),
            None => self.mangle_harness(name),
        }
    }

    /// Resolve a relative class name against [`Self::package`]. Panics
    /// if `name` contains a `.` (a check that catches accidental FQNs in
    /// the relative-name builders). The framework refuses dotted names
    /// on purpose: a binding crate owns one package and must not write
    /// classes into anyone else's namespace. Higher layers wrap or
    /// re-export — they don't get injected into.
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
    pub fn kotlin_ptr_class(mut self, rust_type: syn::Type) -> Self {
        let key = TypeKey::from_type(&rust_type);
        let short = rust_short_name(&key);
        let fqn = self.resolve_class_fqn(&self.mangle_ptr_class(&short));
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
        self
    }

    /// Promote a single `#[prebindgen]` function ident to an instance
    /// method on the generated Kotlin class declared by the most
    /// recent [`Self::kotlin_ptr_class`], [`Self::kotlin_enum`], or
    /// [`Self::kotlin_data_class`] call. For enums/data classes, emitted
    /// methods land in the generated `companion object`. Chain multiple
    /// calls to add multiple methods.
    pub fn method(mut self, method: impl Into<String>) -> Self {
        if let Some(key) = self
            .last_meta_key
            .clone()
            .or_else(|| self.last_opaque_key.clone())
        {
            let entry = self.types.get_mut(&key).expect("type entry vanished");
            entry.methods.push(method.into());
        } else if self.last_package_target {
            self.package_methods.methods.push(method.into());
        } else {
            panic!(
                "JniExt::method must be chained immediately after a `kotlin_ptr_class`, `kotlin_enum`, `kotlin_data_class`, or `kotlin_package` call",
            );
        }
        self
    }

    /// Opt out of Kotlin class emission for the most recent
    /// [`Self::kotlin_ptr_class`] / [`Self::kotlin_enum`] — the `.kt` file is
    /// assumed to be hand-written. Without this, a typed-handle shell
    /// class (or an `enum class`) is auto-emitted. Panics if no
    /// `kotlin_ptr_class` / `kotlin_enum` is in scope.
    pub fn suppress_kotlin_code(mut self) -> Self {
        let key = self.last_meta_key.clone().expect(
            "JniExt::suppress_kotlin_code must be chained immediately after a \
             `kotlin_ptr_class` or `kotlin_enum` call",
        );
        let entry = self.types.get_mut(&key).expect("type entry vanished");
        if let Some(opaque) = entry.opaque.as_mut() {
            opaque.suppress_kotlin_code = true;
        } else if let Some(enum_cfg) = entry.enum_cfg.as_mut() {
            enum_cfg.suppress_kotlin_code = true;
        } else {
            panic!(
                "JniExt::suppress_kotlin_code: type entry for `{}` has neither \
                 `opaque` nor `enum_cfg` set — chain after `kotlin_ptr_class` or \
                 `kotlin_enum`",
                key.as_str()
            );
        }
        self
    }

    /// Whether `ty` was registered via [`Self::kotlin_enum`] — used by
    /// the Kotlin wrapper generator to decide if a parameter needs a
    /// `.value` projection between the typed enum (Kotlin signature) and
    /// the `Int` wire (JNI `external fun`).
    pub(crate) fn is_kotlin_enum(&self, ty: &syn::Type) -> bool {
        let key = TypeKey::from_type(ty);
        self.types
            .get(&key)
            .and_then(|c| c.enum_cfg.as_ref())
            .is_some()
    }

    /// Declare a `#[prebindgen]`-marked `enum` as a Kotlin `enum class`.
    /// Configures: `jni::sys::jint` wire (input + output), `TryFrom<i32>`
    /// decode on input, `as jint` encode on output, and Kotlin enum-file
    /// emission. The enum must be C-like (unit variants only) and either
    /// `#[repr(i32)]` / `#[repr(u8)]` (or similar) with explicit
    /// discriminants — the Kotlin emitter and the generated
    /// `TryFrom<i32>` decode rely on the discriminant values matching the
    /// jint wire.
    ///
    /// By default a `.kt` file is auto-emitted under [`Self::package`]; chain
    /// [`Self::suppress_kotlin_code`] to keep the file hand-maintained.
    /// The class name passes through
    /// [`Self::kotlin_enum_name_mangle`] (default = Rust short name).
    pub fn kotlin_enum(mut self, rust_type: syn::Type) -> Self {
        let key = TypeKey::from_type(&rust_type);
        let short = rust_short_name(&key);
        let fqn = self.resolve_class_fqn(&self.mangle_enum(&short));
        let entry = self.types.entry(key.clone()).or_default();
        assert!(
            entry.opaque.is_none(),
            "JniExt::kotlin_enum: `{}` is already registered as an opaque \
             handle via `kotlin_ptr_class` — a type can be one or the other, \
             not both",
            short
        );
        entry.enum_cfg = Some(EnumConfig::default());
        entry.kotlin_name = Some(fqn.clone());
        self.kotlin_type_fqns
            .push((key.as_str().to_string(), fqn));
        // Clear opaque tracker so a stray `.method(...)` doesn't latch onto
        // this entry; `last_meta_key` is what `.suppress_kotlin_code` reads
        // for chained config.
        self.last_opaque_key = None;
        self.last_meta_key = Some(key);
        self.last_package_target = false;
        self.last_package_target = false;
        self
    }

    /// Stamp a verbatim Kotlin type expression (e.g. `"List<ByteArray>"`)
    /// onto the entry registered by the most recent type-config builder.
    /// Use this when the Kotlin type is not a class FQN (generics,
    /// primitives, container types). For class names, the per-kind
    /// `kotlin_*_name_mangle` closures (configured on [`JniExt`]) own
    /// derivation — `with_kotlin_type` is the escape hatch for verbatim
    /// expressions that don't map onto any one element kind.
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

    /// Install a manual input converter for an `impl Fn(...)` callback
    /// parameter (`JObject` wire). `exc` selects the body convention,
    /// matching the unified [`Self::input_wrapper`] rule:
    ///
    /// * `exc = None` ⇒ non-throwing: emitted body is
    ///   `<dispatcher_path>(env, &v)?` (framework `?`-propagation); only
    ///   valid if the dispatcher returns the framework error.
    /// * `exc = Some(<Rust type>)` ⇒ throwing: the dispatcher is
    ///   expected to return `Result<impl Fn(...), <Rust type>>` (e.g.
    ///   `ZResult<_>`), and the emitted body is the dispatcher call
    ///   directly — no `?`/`Ok`, per the body↔exception coupling. The
    ///   type must match a [`Self::throwable`] declaration
    ///   by exact canonical-form equality (see [`Self::find_exception`]).
    ///
    /// The Kotlin FQN auto-derives via
    /// [`Self::kotlin_callback_name_mangle`] applied to the per-callback
    /// name ([`crate::jni::jni_kotlin_ext::derive_callback_name`]) and
    /// then qualified against [`Self::kotlin_callback_package`]. Set
    /// the mangler closure on [`JniExt`] to control naming (default =
    /// identity).
    pub fn callback_input(
        mut self,
        impl_fn_type: syn::Type,
        exc: Option<syn::Type>,
        dispatcher_path: syn::Path,
    ) -> Self {
        let key = TypeKey::from_type(&impl_fn_type);
        let dispatcher_path_str = dispatcher_path.to_token_stream().to_string();
        let body_path = dispatcher_path_str.clone();
        // `syn::Type` holds `Rc<TokenStream>` internally and is neither
        // `Send` nor `Sync`, so we can't capture it directly in a builder
        // closure that satisfies `WrapperBuilder<Arity0>`'s `Send + Sync`
        // bounds. Serialise to its canonical token form here and re-parse
        // inside the closure — same dance the path captures use.
        let exc_str = exc.as_ref().map(|t| t.to_token_stream().to_string());
        let builder = move |_reg: &Registry<KotlinMeta>| {
            let path: syn::Path = syn::parse_str(&body_path).ok()?;
            // Throwing: dispatcher already returns `Result<_, exc>` — emit
            // the call verbatim. Non-throwing: framework `?`-propagation
            // unwraps, and the framework `Ok`-wraps later.
            let body: syn::Expr = if exc_str.is_some() {
                syn::parse_quote!(#path(env, &v))
            } else {
                syn::parse_quote!(#path(env, &v)?)
            };
            let exc_ty = exc_str.as_deref().and_then(|s| syn::parse_str::<syn::Type>(s).ok());
            Some((
                syn::parse_quote!(jni::objects::JObject),
                exc_ty,
                body,
            ))
        };
        // Auto-derive the callback Kotlin FQN via
        // `kotlin_callback_name_mangle` applied to the per-callback name.
        // Stamped at registration time so downstream consumers
        // (`dispatch_fn_input`, `collect_kotlin_callback_fqns`) read a
        // resolved FQN rather than re-deriving it. The presence of
        // `callback_kotlin_fqn` also flags this entry as a callback for
        // emission paths that need to distinguish.
        let args = crate::core::registry::extract_fn_trait_args(&impl_fn_type)
            .unwrap_or_default();
        let name = crate::jni::jni_kotlin_ext::derive_callback_name(&args);
        let fqn = self.resolve_callback_fqn(&self.mangle_callback(&name));
        let entry = self.types.entry(key.clone()).or_default();
        entry.callback_kotlin_fqn = Some(fqn.clone());
        entry.kotlin_name = Some(fqn.clone());
        self.kotlin_type_fqns
            .push((key.as_str().to_string(), fqn));
        self.input_wrappers[0].insert(key.clone(), builder.into_wrapper_fn());
        self.note_wrapper_registration(key, 0);
        self
    }

    /// Mark an `impl Fn(...)` callback type as having a hand-written
    /// Kotlin fun-interface. The framework keeps its default Rust-side
    /// auto-dispatcher (no [`Self::callback_input`] override here) but
    /// skips emitting the Kotlin auto-stub — the binding crate provides
    /// the `<FQN>.kt` file itself. The Kotlin FQN is auto-derived via
    /// [`Self::mangle_callback`] applied to the callback's name so the
    /// hand-written file name and the JNI-side mention stay in sync.
    /// Equivalent to chaining `.suppress_kotlin_code()` after a
    /// [`Self::kotlin_ptr_class`] / [`Self::kotlin_enum`] declaration, but
    /// inline because callbacks don't have a `kotlin_callback` builder
    /// to chain off.
    pub fn suppress_kotlin_callback_code(mut self, impl_fn_type: syn::Type) -> Self {
        let key = TypeKey::from_type(&impl_fn_type);
        let args = crate::core::registry::extract_fn_trait_args(&impl_fn_type)
            .unwrap_or_default();
        let name = crate::jni::jni_kotlin_ext::derive_callback_name(&args);
        let fqn = self.resolve_callback_fqn(&self.mangle_callback(&name));
        let entry = self.types.entry(key.clone()).or_default();
        entry.callback_kotlin_fqn = Some(fqn.clone());
        entry.kotlin_name = Some(fqn.clone());
        self.kotlin_type_fqns
            .push((key.as_str().to_string(), fqn));
        self.last_opaque_key = None;
        self.last_meta_key = None;
        self.last_package_target = false;
        self
    }

    /// Declare a Rust struct that should appear in Kotlin as a data
    /// class under a derived name. The name passes through
    /// [`Self::kotlin_data_class_name_mangle`] (default = Rust short
    /// name, generics / lifetimes stripped). Only affects Kotlin
    /// emission — no Rust-side converter override.
    pub fn kotlin_data_class(mut self, rust_type: syn::Type) -> Self {
        let key = TypeKey::from_type(&rust_type);
        let short = rust_short_name(&key);
        let fqn = self.resolve_class_fqn(&self.mangle_data_class(&short));
        let entry = self.types.entry(key.clone()).or_default();
        entry.kotlin_name = Some(fqn.clone());
        self.kotlin_type_fqns
            .push((key.as_str().to_string(), fqn));
        self.last_opaque_key = None;
        self.last_meta_key = Some(key);
        self.last_package_target = false;
        self
    }

    /// Register `impl Into<target>` source arms. `target_key` is the
    /// canonical Rust type (e.g. `"ZKeyExpr<'static>"`); `sources` is
    /// an ordered list of [`IntoSource`] arms (dispatch order matches
    /// iteration order).
    pub fn into_sources<I>(mut self, target_type: syn::Type, sources: I) -> Self
    where
        I: IntoIterator<Item = IntoSource>,
    {
        let key = TypeKey::from_type(&target_type);
        self.into_sources_map
            .insert(key, sources.into_iter().collect());
        self.last_opaque_key = None;
        self.last_meta_key = None;
        self.last_package_target = false;
        self
    }

    /// Register a rank-N **input converter**. `pattern` contains 0–3
    /// `_` placeholders; the closure's arity selects the rank table.
    /// The closure returns `Some((ty, exc, body))` (see [`WrapperFn`]
    /// for the triple's full semantics) or `None` (defer to a later
    /// resolver phase). The body sees `env: &mut JNIEnv` and `v: &<wire>`
    /// in scope.
    ///
    /// * `exc = None` ⇒ non-throwing: `body` evaluates to a bare `ty`;
    ///   framework emits `-> Result<ty, __JniErr>` with an `Ok(...)`
    ///   wrap, and `?` inside propagates the framework error.
    /// * `exc = Some(<Rust type>)` ⇒ throwing: `body` evaluates to
    ///   `Result<ty, <Rust type>>`; framework emits it verbatim. The
    ///   type must match a [`Self::throwable`] declaration
    ///   by **exact canonical-form equality** with its `rust_type` (see
    ///   [`Self::find_exception`] — no short-name fallback). The match
    ///   is validated at lookup time.
    ///
    /// `ty` is auto-classified at resolve: a wire shape ⇒ terminal
    /// converter; a distinct rust type with its own converter ⇒ a
    /// value-inspecting stage composed onto that converter's chain
    /// (see [`Self::lookup_input`]).
    pub fn input_wrapper<A, B>(self, pattern: syn::Type, builder: B) -> Self
    where
        B: WrapperBuilder<A>,
    {
        let key = TypeKey::from_type(&pattern);
        let rank = B::rank();
        let mut s = self;
        s.input_wrappers[rank].insert(key.clone(), builder.into_wrapper_fn());
        s.note_wrapper_registration(key, rank);
        s
    }

    /// Output-direction counterpart of [`Self::input_wrapper`]. Same
    /// closure shape, same `exc = None` / `Some(<Rust type>)` semantics,
    /// same terminal-vs-composed classification — see that method's docs.
    /// (`Some(parse_quote!(<full path>))` with a rust-typed `ty`, e.g.
    /// `(T, Some(parse_quote!(zenoh_flat::errors::ZError)), v)` for
    /// `ZResult<T>`, gives the auto-composed peel that the deleted
    /// `output_throw_stage` used to register.)
    pub fn output_wrapper<A, B>(self, pattern: syn::Type, builder: B) -> Self
    where
        B: WrapperBuilder<A>,
    {
        let key = TypeKey::from_type(&pattern);
        let rank = B::rank();
        let mut s = self;
        s.output_wrappers[rank].insert(key.clone(), builder.into_wrapper_fn());
        s.note_wrapper_registration(key, rank);
        s
    }

    /// Shared post-registration bookkeeping for wrapper inserts. Rank-0
    /// patterns identify a concrete type — auto-stamp `kotlin_name` via
    /// [`Self::mangle_wrapper`] (skipping callback entries, whose
    /// `kotlin_name` is already stamped via
    /// [`Self::mangle_callback`] in [`Self::callback_input`], and
    /// non-path patterns like `()` where there is no sensible short
    /// name). Rank ≥1 patterns are wildcards — per-outer-type names
    /// come from inner-metadata propagation via
    /// [`Self::override_kotlin_name`].
    fn note_wrapper_registration(&mut self, key: TypeKey, rank: usize) {
        self.last_opaque_key = None;
        self.last_package_target = false;
        if rank == 0 {
            let entry = self.types.entry(key.clone()).or_default();
            // Skip callbacks (handled by callback_input) and any entry
            // whose kotlin_name has already been stamped (e.g. by an
            // earlier kotlin_data_class / kotlin_ptr_class call for the
            // same type — a wrapper layered on top should not override
            // it). Then derive the short name from the canonical
            // TypeKey; non-path patterns ($()$, references, etc.)
            // yield no Kotlin class name and are left as `None`.
            if entry.kotlin_name.is_none() && entry.callback_kotlin_fqn.is_none() {
                if let Some(short) = rust_short_name_opt(&key) {
                    let fqn = self.resolve_class_fqn(&self.mangle_wrapper(&short));
                    let entry = self.types.get_mut(&key).expect("just-inserted entry");
                    entry.kotlin_name = Some(fqn.clone());
                    self.kotlin_type_fqns
                        .push((key.as_str().to_string(), fqn));
                }
            }
            self.last_meta_key = Some(key);
        } else {
            self.last_meta_key = None;
        }
    }

    /// [`Self::find_exception`] with a uniform fail-fast panic. `who` is
    /// the caller name for the message.
    fn find_exception_or_panic(&self, who: &str, ty: &syn::Type) -> usize {
        self.find_exception(ty).unwrap_or_else(|| {
            let needle = ty.to_token_stream().to_string();
            panic!(
                "JniExt::{who}: no exception class registered matching `{needle}` — \
                 declare it via `.kotlin_data_class(<rust path>).throwable()` (or the \
                 ptr/enum equivalent) first, and bind closures to it with \
                 `Some(parse_quote!(<the same path>))`. The framework default is \
                 `::prebindgen_ext::jni::JniBindingError` (or omit the closure's middle \
                 slot — pass `None` — for non-throwing)."
            )
        })
    }

    /// Resolve an exception type against the registered
    /// [`Self::exceptions`] by **exact canonical-form equality** with the
    /// declaration's `rust_type`. No short-name fallback — the closure /
    /// caller must spell the same full path
    /// `.throwable()` declared the type with. Returns the
    /// index into the `exceptions` vec on match.
    fn find_exception(&self, ty: &syn::Type) -> Option<usize> {
        let needle = ty.to_token_stream().to_string();
        self.exceptions.iter().position(|e| {
            e.rust_type.to_token_stream().to_string() == needle
        })
    }

    /// The framework's pre-registered [`crate::jni::JniBindingError`]
    /// exception. Always exists at `exceptions[0]` after [`Self::new`].
    pub(crate) fn framework_exception(&self) -> &ExceptionConfig {
        &self.exceptions[0]
    }

    /// Build a `KotlinMeta` stamped with the framework's
    /// `JniBindingError` as the converter's *thrown JVM class*. Used by
    /// every built-in fallible converter (primitives, structs,
    /// `Option<_>`, `Vec<_>`, callbacks). Both fields point at the
    /// framework exception:
    ///   * `throws` (FQN) → the Kotlin `@Throws(...)` aggregator;
    ///   * `throws_action` (`throw_JniBindingError`) → the throw fn the
    ///     function wrapper calls when this converter's `?` fails.
    /// The Rust error value flowing in is the unified `__JniErr`
    /// (domain error type), but `throw_JniBindingError` is generic over
    /// `Display`, so it surfaces that value as `JniBindingError` on the
    /// JVM regardless of the value's Rust type. (Bare-wire vs `Result`
    /// output converters are discriminated by signature inspection
    /// [`converter_returns_result`], not by `throws_action`.)
    pub(crate) fn framework_meta(&self, kotlin_name: Option<String>) -> KotlinMeta {
        let exc = self.framework_exception();
        KotlinMeta {
            kotlin_name,
            throws: Some(exc.kotlin_fqn.clone()),
            throws_action: Some(exception_throw_path(exc)),
            value_rust_key: None,
            handle: None,
        }
    }

    // ── Wrapper-table lookups (used by PrebindgenExt impl) ───────────

    /// Look up a registered input converter for `pat` with `args`
    /// substituted into its `_` slots. The closure's middle slot (see
    /// [`WrapperFn`]) carries the bound exception — `None` ⇒ framework
    /// `__JniErr` with an `Ok`-wrap, `Some(<Rust type>)` ⇒
    /// `Result<ty, <Rust type>>` emitted verbatim, decided in
    /// [`Self::build_input_fn`].
    ///
    /// The closure's returned type is classified by [`is_wire_type`]:
    /// * **wire** ⇒ terminal: a single converter `wire → outer`.
    /// * **rust type** ⇒ composed: that type's input converter runs
    ///   first (`wire → ty`), then this registration's body is a
    ///   value-inspecting stage `ty → outer` (built by-value via
    ///   [`Self::build_output_fn`]) prepended to the inner chain. Defer
    ///   (`None`) if the inner converter isn't resolved yet.
    pub(crate) fn lookup_input(
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
        let f = self.input_wrappers[rank].get(&key)?;
        let (ty, exc_ty, body) = f(args, registry)?;
        // Resolve the exception type lazily: validated here, at lookup
        // time, rather than at the `input_wrapper` call site — the
        // closure is the single source of truth for both body shape and
        // bound exception (see [`WrapperFn`]).
        let exc = exc_ty
            .as_ref()
            .map(|t| &self.exceptions[self.find_exception_or_panic("input_wrapper", t)]);
        let outer = substitute_wildcards(pat, args);
        let throw_exc = exc.unwrap_or_else(|| self.framework_exception());
        // Terminal vs composed: `ty` is composed iff it's a *distinct*
        // rust type with its own input converter. The self-check guards
        // the void/identity case (`output_wrapper("()")` returns `ty ==
        // outer`), and the registered-converter probe distinguishes a
        // rust continue-type (compose) from a wire (terminal) without
        // forcing `()` either way. A non-wire `ty` that isn't yet
        // resolved defers.
        let is_self = TypeKey::from_type(&ty) == TypeKey::from_type(&outer);
        let inner = if is_self { None } else { registry.input_entry(&ty) };
        match inner {
            None if is_self || is_wire_type(&ty) => {
                // Terminal: `ty` is the wire; the body produces `outer`.
                let (niches, kotlin_name) = if rank == 0 {
                    let kn = self
                        .types
                        .get(&key)
                        .and_then(|c| c.kotlin_name.clone())
                        .or_else(|| crate::jni::jni_kotlin_ext::kotlin_for_wire(&ty));
                    (Niches::empty(), kn)
                } else {
                    (default_niches_for_wire(&ty), None)
                };
                Some(ConverterImpl {
                    pre_stages: vec![],
                    function: self.build_input_fn(&outer, &ty, &body, exc),
                    destination: ty,
                    niches,
                    metadata: KotlinMeta {
                        kotlin_name,
                        throws: Some(throw_exc.kotlin_fqn.clone()),
                        throws_action: Some(exception_throw_path(throw_exc)),
                        value_rust_key: None,
                        // Terminal: body produces the wire directly, no inner
                        // converter composed, so no handle to carry.
                        handle: None,
                    },
                })
            }
            // Non-wire `ty` whose converter isn't resolved yet — defer.
            None => None,
            Some(inner) => {
                // Composed: `ty` is the inner source rust type. Its input
                // converter (`wire → ty`) is the wire-facing function;
                // this body is a stage `ty → outer` that runs after it.
                // The stage takes the inner-produced value BY VALUE and
                // yields `outer`, i.e. the same shape an output converter
                // has — so it's built with `build_output_fn`.
                let stage = Stage {
                    function: self.build_output_fn(&ty, &outer, &body, exc),
                    throws_fqn: throw_exc.kotlin_fqn.clone(),
                    throws_action: exception_throw_path(throw_exc),
                };
                let mut pre_stages = vec![stage];
                pre_stages.extend(inner.pre_stages.iter().cloned());
                let (kotlin_name, value_rust_key) = if rank >= 1 {
                    (
                        inner.metadata.kotlin_name.clone(),
                        Some(TypeKey::from_type(&args[0]).as_str().to_string()),
                    )
                } else {
                    (inner.metadata.kotlin_name.clone(), None)
                };
                let niches = if rank == 0 {
                    Niches::empty()
                } else {
                    default_niches_for_wire(&inner.destination)
                };
                Some(ConverterImpl {
                    function: inner.function.clone(),
                    destination: inner.destination.clone(),
                    pre_stages,
                    niches,
                    metadata: KotlinMeta {
                        kotlin_name,
                        throws: inner.metadata.throws.clone(),
                        throws_action: inner.metadata.throws_action.clone(),
                        value_rust_key,
                        // Identity propagation: a composed wrapper (e.g.
                        // `Result<Handle,Error>`) projects to its inner value,
                        // so a handle inner stays a handle (same strategy).
                        handle: inner.metadata.handle.clone(),
                    },
                })
            }
        }
    }

    /// Look up a registered output converter for `pat` with `args`
    /// substituted into its `_` slots. Mirror of [`Self::lookup_input`].
    ///
    /// The closure's returned type is classified by [`is_wire_type`]:
    /// * **wire** ⇒ terminal: a single converter `outer → wire`,
    ///   returning `Result<wire, err>` (throwing iff [`ConverterReg::exc`]
    ///   is set).
    /// * **rust type** ⇒ composed: this body is a value-inspecting stage
    ///   `outer → ty` prepended to `ty`'s own output converter chain
    ///   (e.g. `ZResult<T>` returns rust `T`, so the peel stage raises
    ///   its exception and `T`'s converter marshals the wire). Defer
    ///   (`None`) if `ty`'s converter isn't resolved yet.
    pub(crate) fn lookup_output(
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
        let f = self.output_wrappers[rank].get(&key)?;
        let (ty, exc_ty, body) = f(args, registry)?;
        // Resolve at lookup — see [`Self::lookup_input`] for the rationale.
        let exc = exc_ty
            .as_ref()
            .map(|t| &self.exceptions[self.find_exception_or_panic("output_wrapper", t)]);
        let outer = substitute_wildcards(pat, args);
        let throw_exc = exc.unwrap_or_else(|| self.framework_exception());
        // Terminal vs composed — see [`Self::lookup_input`] for the rule.
        let is_self = TypeKey::from_type(&ty) == TypeKey::from_type(&outer);
        let inner = if is_self { None } else { registry.output_entry(&ty) };
        match inner {
            None if is_self || is_wire_type(&ty) => {
                // Terminal: `ty` is the wire; the body produces it from `outer`.
                let (kotlin_name, value_rust_key) = if rank >= 1 {
                    registry
                        .output_entry(&args[0])
                        .map(|e| {
                            (
                                e.metadata.kotlin_name.clone(),
                                Some(TypeKey::from_type(&args[0]).as_str().to_string()),
                            )
                        })
                        .unwrap_or((None, None))
                } else {
                    let kn = self
                        .types
                        .get(&key)
                        .and_then(|c| c.kotlin_name.clone())
                        .or_else(|| crate::jni::jni_kotlin_ext::kotlin_for_wire(&ty));
                    (kn, None)
                };
                let niches = if rank == 0 {
                    Niches::empty()
                } else {
                    default_niches_for_wire(&ty)
                };
                Some(ConverterImpl {
                    pre_stages: vec![],
                    function: self.build_output_fn(&outer, &ty, &body, exc),
                    destination: ty,
                    niches,
                    metadata: KotlinMeta {
                        kotlin_name,
                        throws: Some(throw_exc.kotlin_fqn.clone()),
                        throws_action: Some(exception_throw_path(throw_exc)),
                        value_rust_key,
                        // Terminal: body produces the wire directly, no inner
                        // converter composed, so no handle to carry.
                        handle: None,
                    },
                })
            }
            // Non-wire `ty` whose converter isn't resolved yet — defer.
            None => None,
            Some(inner) => {
                // Composed: `ty` is the continue rust type; chain its converter.
                let stage = Stage {
                    function: self.build_output_fn(&outer, &ty, &body, exc),
                    throws_fqn: throw_exc.kotlin_fqn.clone(),
                    throws_action: exception_throw_path(throw_exc),
                };
                let mut pre_stages = vec![stage];
                pre_stages.extend(inner.pre_stages.iter().cloned());
                let (kotlin_name, value_rust_key) = if rank >= 1 {
                    (
                        inner.metadata.kotlin_name.clone(),
                        Some(TypeKey::from_type(&args[0]).as_str().to_string()),
                    )
                } else {
                    (inner.metadata.kotlin_name.clone(), None)
                };
                let niches = if rank == 0 {
                    Niches::empty()
                } else {
                    default_niches_for_wire(&inner.destination)
                };
                Some(ConverterImpl {
                    function: inner.function.clone(),
                    destination: inner.destination.clone(),
                    pre_stages,
                    niches,
                    metadata: KotlinMeta {
                        kotlin_name,
                        throws: inner.metadata.throws.clone(),
                        throws_action: inner.metadata.throws_action.clone(),
                        value_rust_key,
                        // Identity propagation: a composed wrapper (e.g.
                        // `Result<Handle,Error>`) projects to its inner value,
                        // so a handle inner stays a handle (same strategy).
                        handle: inner.metadata.handle.clone(),
                    },
                })
            }
        }
    }
}

/// Recognise the JNI **wire** shapes a converter body may return as a
/// terminal destination. Reuses the back-end's existing wire knowledge:
/// every `jni::sys::*` / `jni::objects::*` wire is recognised by
/// [`crate::jni::jni_kotlin_ext::kotlin_for_wire`] (returns `Some`), plus
/// raw pointers structurally — so there is no separate wire-type
/// allowlist to keep in sync.
///
/// `()` is deliberately **not** treated as a wire here: it is ambiguous
/// (the void wire of the `output_wrapper("()")` self-converter *and* the
/// unit continue-type of `ZResult<()>`). The terminal-vs-composed
/// decision in [`JniExt::lookup_input`] / [`JniExt::lookup_output`]
/// resolves that ambiguity via the self-check + registered-converter
/// probe, so `()` flows correctly without being force-classified here.
fn is_wire_type(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Ptr(_)) || crate::jni::jni_kotlin_ext::kotlin_for_wire(ty).is_some()
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

/// Bare-ident type `__JniErr` — the generated file's alias for the
/// framework `JniBindingError`. Non-throwing converters use this as
/// their `Result<…, _>` error type so their bodies' `<__JniErr as
/// From<String>>::from(...)` calls keep compiling, and so a
/// `?`-propagated framework failure surfaces as the framework
/// exception on the JVM. Throwing converters bypass this in favour of
/// their bound exception's own type (see [`JniExt::lookup_input`] /
/// [`JniExt::lookup_output`]). Returned as `syn::Type` so it shares the
/// `err_type` binding with [`ExceptionConfig::rust_type`].
pub(crate) fn default_err_type() -> syn::Type {
    syn::parse_quote!(__JniErr)
}

/// The body expression to splice into a converter `fn` returning
/// `Result<_, E>`, per the body↔exception coupling: a non-throwing
/// converter's `body` is a bare value, so wrap it `Ok(body)`; a throwing
/// converter's `body` already evaluates to the `Result`, so emit it
/// verbatim. (Replaces the old "always-`Ok`-wrap then strip" dance.)
fn body_for_exc(body: &syn::Expr, exc: Option<&ExceptionConfig>) -> syn::Expr {
    if exc.is_some() {
        body.clone()
    } else {
        syn::parse_quote!(Ok(#body))
    }
}

/// Construct an [`ExceptionConfig`] from a path-shaped `syn::Type` and
/// the current Kotlin package. Shared by [`JniExt::new`] (framework
/// `JniBindingError` slot) and [`JniExt::throwable`] (user-declared slots).
/// (user-declared slots). Panics if `rust_type` isn't a `Type::Path`
/// or if its short-name collides with an already-registered exception.
fn build_exception_config(
    rust_type: syn::Type,
    package: &str,
    existing: &[ExceptionConfig],
) -> ExceptionConfig {
    let segs = match &rust_type {
        syn::Type::Path(tp) => &tp.path.segments,
        _ => panic!(
            "throwable: expected a path-shaped type, got `{}`",
            rust_type.to_token_stream()
        ),
    };
    let short = segs
        .last()
        .map(|s| s.ident.to_string())
        .unwrap_or_else(|| {
            panic!(
                "throwable: rust type `{}` has no path segments",
                rust_type.to_token_stream()
            )
        });
    let kotlin_fqn = if package.is_empty() {
        short.clone()
    } else {
        format!("{}.{}", package, short)
    };
    let throw_fn_name = format_ident!("throw_{}", short);
    if existing.iter().any(|e| e.throw_fn_name == throw_fn_name) {
        panic!(
            "throwable: another exception is already \
             registered with Rust short name `{}` — rename the Rust \
             type to disambiguate",
            short
        );
    }
    ExceptionConfig {
        rust_type,
        rust_short: short,
        kotlin_fqn,
        throw_fn_name,
    }
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
    ///
    /// `exc` ties the body convention to the bound exception:
    /// * `None` (non-throwing) → signature `Result<rust, __JniErr>` and
    ///   the body is wrapped `Ok(<body>)`; `?` inside propagates the
    ///   framework error.
    /// * `Some(X)` (throwing) → signature `Result<rust, X::rust_type>`
    ///   and the body is emitted as-is — `<body>` already evaluates to
    ///   that `Result`, so no `Ok` wrap (and no cross-type `From`).
    pub(crate) fn build_input_fn(
        &self,
        rust: &syn::Type,
        wire: &syn::Type,
        body: &syn::Expr,
        exc: Option<&ExceptionConfig>,
    ) -> syn::ItemFn {
        let name = input_name(rust, wire);
        let rust_with_lifetime = annotate_borrow_with_lifetime(rust, "env");
        let wire_with_lifetime = annotate_jobject_with_lifetime(wire, "v");
        let err_type = exc.map(|e| e.rust_type.clone()).unwrap_or_else(default_err_type);
        let ret_body = body_for_exc(body, exc);
        if matches!(wire, syn::Type::Ptr(_)) {
            syn::parse_quote!(
                #[allow(non_snake_case, unused_mut, unused_variables, unused_braces, dead_code)]
                pub(crate) unsafe fn #name<'env>(env: &mut jni::JNIEnv<'env>, v: #wire) -> ::core::result::Result<#rust_with_lifetime, #err_type> {
                    #ret_body
                }
            )
        } else {
            syn::parse_quote!(
                #[allow(non_snake_case, unused_mut, unused_variables, unused_braces, dead_code)]
                pub(crate) unsafe fn #name<'env, 'v>(env: &mut jni::JNIEnv<'env>, v: &#wire_with_lifetime) -> ::core::result::Result<#rust_with_lifetime, #err_type> {
                    #ret_body
                }
            )
        }
    }

    /// Build the standard JNI output-converter `fn`. Body assumes in-scope
    /// `env: &mut JNIEnv` and `v: <rust>` (by value — handles like
    /// `Subscriber<()>` aren't `Clone`, so callers move into the converter).
    ///
    /// `exc` — see [`Self::build_input_fn`]; same body↔exception coupling,
    /// output side.
    pub(crate) fn build_output_fn(
        &self,
        rust: &syn::Type,
        wire: &syn::Type,
        body: &syn::Expr,
        exc: Option<&ExceptionConfig>,
    ) -> syn::ItemFn {
        let name = output_name(rust, wire);
        let wire_with_lifetime = annotate_jobject_with_lifetime(wire, "a");
        let err_type = exc.map(|e| e.rust_type.clone()).unwrap_or_else(default_err_type);
        let ret_body = body_for_exc(body, exc);
        syn::parse_quote!(
            #[allow(non_snake_case, unused_mut, unused_variables, unused_braces, dead_code)]
            pub(crate) unsafe fn #name<'a>(env: &mut jni::JNIEnv<'a>, v: #rust) -> ::core::result::Result<#wire_with_lifetime, #err_type> {
                #ret_body
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
            pre_stages: vec![],
            niches: Niches::one(
                syn::parse_quote!(0i64),
                syn::parse_quote!(*v == 0),
            ),
            // Opaque handles' value-context Kotlin name stays `"Long"`
            // (the jlong wire mention); the *typed* Kotlin rendering is
            // derived from `handle` below. The wrapper's `?` path surfaces
            // an `OwnedObject::from_raw` failure as the framework
            // `JniBindingError`, so the throws fields point at the
            // framework exception.
            metadata: self.opaque_leaf_meta(ty),
        }
    }

    /// Leaf metadata for an opaque handle: value-context name `"Long"`
    /// plus the [`HandleInfo`] that folds outward through wrappers (owned,
    /// [`CloseStrategy::Direct`]). The single seam where a Rust type is
    /// first marked a closeable native handle.
    fn opaque_leaf_meta(&self, ty: &syn::Type) -> KotlinMeta {
        KotlinMeta {
            handle: Some(HandleInfo {
                leaf_key: TypeKey::from_type(ty).as_str().to_string(),
                owned: true,
                strategy: CloseStrategy::Direct,
            }),
            ..self.framework_meta(Some("Long".to_string()))
        }
    }

    /// If the user pinned a Kotlin name for `outer_ty` via
    /// [`Self::kotlin_data_class`] (or it's an opaque-handle entry that
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
    /// the converter's [`KotlinMeta`] at creation time. The relative
    /// class name passes through [`Self::mangle_callback`] before
    /// being qualified against
    /// [`Self::kotlin_callback_package`].
    pub(crate) fn auto_callback_fqn(&self, args: &[syn::Type]) -> String {
        let name = crate::jni::jni_kotlin_ext::derive_callback_name(args);
        self.resolve_callback_fqn(&self.mangle_callback(&name))
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

    fn emitted_source_type_names(&self) -> std::collections::HashSet<String> {
        let mut names = std::collections::HashSet::new();
        for key in self.types.keys() {
            if let Some(short) = rust_short_name_opt(key) {
                names.insert(short);
            }
        }
        for exc in self.exceptions.iter().skip(1) {
            if let Some(short) = type_last_ident(&exc.rust_type) {
                names.insert(short.to_string());
            }
        }
        names
    }

    /// Walk `item` and prefix every bare single-segment type reference
    /// matching a [`Self::emitted_source_type_names`] name with
    /// [`Self::source_module`]. Applied once per emitted item at write
    /// time via [`PrebindgenExt::post_process_item`] so converter bodies,
    /// type ascriptions, and casts all stay in sync without each emit
    /// site having to remember to qualify.
    fn qualify_item(&self, item: &mut syn::Item) {
        let source_names = self.emitted_source_type_names();
        if source_names.is_empty() {
            return;
        }
        let mut visitor = QualifyEmittedTypes {
            source_module: &self.source_module,
            source_names: &source_names,
        };
        syn::visit_mut::VisitMut::visit_item_mut(&mut visitor, item);
    }

    /// Output side of [`Self::opaque_handle_input`] — see that method's
    /// docs for the full convention.
    pub fn opaque_handle_output(&self, ty: &syn::Type) -> ConverterImpl<KotlinMeta> {
        let wire: syn::Type = syn::parse_quote!(jni::sys::jlong);
        let body: syn::Expr = syn::parse_quote!(
            std::boxed::Box::into_raw(std::boxed::Box::new(v)) as i64
        );
        ConverterImpl {
            function: self.build_output_fn(ty, &wire, &body, None),
            destination: wire,
            pre_stages: vec![],
            niches: Niches::one(
                syn::parse_quote!(0i64),
                syn::parse_quote!(*v == 0),
            ),
            // Opaque handles' value-context name `"Long"` + folded
            // `HandleInfo` — see [`Self::opaque_handle_input`] /
            // [`Self::opaque_leaf_meta`]. Framework throws because the
            // wrapper's emitted match-arm still has a `JniBindingError`
            // branch reachable via the chain.
            metadata: self.opaque_leaf_meta(ty),
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
            pre_stages: vec![],
            niches: Niches::empty(),
            // `impl Into<T>` parameters surface as Kotlin `Any` — the
            // safe wrapper does an `is JNI<X>` chain on the value, and
            // the JNI dispatcher's matching arm uses each source's
            // typed FQN under the hood. The dispatcher's per-arm `?`
            // decode + no-match `Err` fallthrough can fail, so it
            // carries the framework throws.
            metadata: self.framework_meta(Some("Any".to_string())),
        })
    }

}

/// One `pub(crate) fn throw_<short>(...)` item for an exception.
/// Emitted from [`PrebindgenExt::prerequisites`] so it lands at the
/// top of the same generated file as every other converter — wrapper
/// code below can call it by bare name (`throw_<short>(env, &err)`);
/// hand-written modules in the binding crate reach it via the include
/// module path (e.g. `crate::generated::throw_<short>`). The body
/// finds the JVM class by slash-form FQN and `throw_new`s with
/// `err.to_string()`, logging on either failure.
///
/// The error parameter is generic over `Display` rather than the
/// exception's own Rust type. This decouples the *thrown JVM class*
/// from the *Rust error value*: the unified converter error type
/// (`__JniErr`, the binding's primary domain error) flows through
/// every converter, but each converter chooses which `throw_<short>`
/// to call — so a built-in decode failure carries the domain error
/// value yet surfaces on the JVM as `JniBindingError`. (It also avoids
/// any cross-crate `From` bridge between the framework error type and
/// the domain error type, which the crate layering forbids.)
fn build_throw_fn_item(
    ext: &JniExt,
    registry: &Registry<KotlinMeta>,
    exc: &ExceptionConfig,
) -> syn::Item {
    let throw_fn = &exc.throw_fn_name;
    let class_path_slashes = exc.kotlin_fqn.replace('.', "/");
    // Structured path: when the exception's Rust type has its own
    // registered output converter (i.e. `.kotlin_data_class(...).throwable()`),
    // construct the JVM object via that converter and throw it as the
    // type's own JVM class — so a structured error carries its fields
    // across the boundary, not just `Display::to_string`. Requires the
    // type to be `Clone` (the converter consumes `v` by value).
    let key = TypeKey::from_type(&exc.rust_type);
    let is_data_class = ext
        .types
        .get(&key)
        .map(|cfg| {
            cfg.kotlin_name.is_some()
                && cfg.opaque.is_none()
                && cfg.enum_cfg.is_none()
                && cfg.callback_kotlin_fqn.is_none()
                && cfg.throwable
        })
        .unwrap_or(false);
    let output_conv = if is_data_class {
        registry
            .output_entry(&exc.rust_type)
            .map(|e| e.function.sig.ident.clone())
    } else {
        None
    };
    if let Some(conv) = output_conv {
        let rust_ty = &exc.rust_type;
        let class_short = &exc.rust_short;
        return syn::parse_quote!(
            #[allow(non_snake_case)]
            pub(crate) fn #throw_fn(env: &mut jni::JNIEnv, err: &#rust_ty) {
                let jobj = match unsafe { #conv(env, err.clone()) } {
                    Ok(o) => o,
                    Err(e) => {
                        tracing::error!(
                            "Failed to encode {} for throw: {}",
                            #class_short,
                            e
                        );
                        return;
                    }
                };
                let throwable = jni::objects::JThrowable::from(jobj);
                if let Err(e) = env.throw(throwable) {
                    tracing::error!("Failed to throw exception: {}", e);
                }
            }
        );
    }
    // Display path: framework `JniBindingError` (no `#[prebindgen]`,
    // no data class — just a class name + a Display message).
    syn::parse_quote!(
        #[allow(non_snake_case)]
        pub(crate) fn #throw_fn(
            env: &mut jni::JNIEnv,
            err: &(impl ::core::fmt::Display + ?Sized),
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

/// One `#[no_mangle] extern "C"` destructor per non-suppressed opaque
/// handle — the Rust counterpart to the `public fun free() = free {
/// freePtr<suffix>(it) }` / `private external fun freePtr<suffix>` pair
/// emitted by [`render_typed_handle_source`]. Each body is the uniform
/// `drop(Box::from_raw(ptr as *mut T))`; the inner `T`'s own `Drop` runs
/// (e.g. `Publisher` network-undeclare) with no special casing.
///
/// Emitted under the same `opaque && !suppress_kotlin_code` condition as
/// the Kotlin shell, so the framework owns *both* halves of the
/// destructor exactly when it owns the typed-handle class. Suppressed
/// handles (hand-written Kotlin) keep their hand-written Rust destructor.
///
/// The symbol follows the documented scheme
/// `Java_<package_underscores>_<class_short>_<mangle_fun("freePtr")>`,
/// where `class_short` is the last segment of the typed-handle FQN
/// (`TypeConfig::kotlin_name`) and the `freePtr` name passes through
/// [`JniExt::mangle_fun`] — exact symmetry with the Kotlin
/// `external fun <mangle_fun("freePtr")>` declaration in
/// [`render_typed_handle_source`]. `ext.types` is a `HashMap`, so the
/// items are sorted by symbol to keep generated output deterministic.
///
/// Emission is gated on the resolved `registry`: a destructor is only
/// emitted for an opaque handle whose type a scanned `#[prebindgen]` fn
/// actually references (as input or output). This mirrors converter
/// emission and keeps feature-gated handles (e.g. `zenoh-ext`-only types
/// whose declare/undeclare fns are `#[cfg]`'d out of the scan) from
/// producing destructors that reference types not in scope.
fn build_handle_destructor_items(
    ext: &JniExt,
    registry: &Registry<KotlinMeta>,
) -> Vec<syn::Item> {
    let pkg = ext.package.replace('.', "_");
    let free_ptr = ext.mangle_fun("freePtr");
    let mut named: Vec<(String, syn::Item)> = Vec::new();
    for (key, cfg) in &ext.types {
        let Some(opaque) = &cfg.opaque else { continue };
        if opaque.suppress_kotlin_code {
            continue;
        }
        // Skip handles the (feature-aware) scan never references — their
        // type may not be in scope in the generated module.
        let ty = key.to_type();
        if registry.input_entry(&ty).is_none() && registry.output_entry(&ty).is_none() {
            continue;
        }
        let class_short = cfg
            .kotlin_name
            .as_deref()
            .and_then(|fqn| fqn.rsplit('.').next())
            .unwrap_or_else(|| {
                panic!(
                    "build_handle_destructor_items: opaque handle `{}` has no \
                     kotlin_name to derive a destructor symbol from",
                    key.as_str()
                )
            });
        let symbol = if pkg.is_empty() {
            format!("Java_{class_short}_{free_ptr}")
        } else {
            format!("Java_{pkg}_{class_short}_{free_ptr}")
        };
        let ident = syn::Ident::new(&symbol, Span::call_site());
        let item: syn::Item = syn::parse_quote!(
            #[no_mangle]
            #[allow(non_snake_case, unused_variables)]
            pub(crate) unsafe extern "C" fn #ident(
                _env: jni::JNIEnv,
                _class: jni::objects::JClass,
                ptr: jni::sys::jlong,
            ) {
                if ptr != 0 {
                    drop(Box::from_raw(ptr as *mut #ty));
                }
            }
        );
        named.push((symbol, item));
    }
    named.sort_by(|a, b| a.0.cmp(&b.0));
    named.into_iter().map(|(_, item)| item).collect()
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

    /// Union of every per-class `.method(...)` list and the package
    /// object's method list. Each entry is a `#[prebindgen]` fn ident
    /// the user explicitly hooked into the binding; functions not in
    /// this set are skipped by the registry's signature scan and by
    /// the per-item emitter.
    fn declared_functions(&self) -> std::collections::HashSet<syn::Ident> {
        let mut out = std::collections::HashSet::new();
        for cfg in self.types.values() {
            for name in &cfg.methods {
                if let Ok(ident) = syn::parse_str::<syn::Ident>(name) {
                    out.insert(ident);
                }
            }
        }
        for name in &self.package_methods.methods {
            if let Ok(ident) = syn::parse_str::<syn::Ident>(name) {
                out.insert(ident);
            }
        }
        out
    }

    /// Every type registered via `.kotlin_ptr_class`,
    /// `.kotlin_data_class`, or `.kotlin_enum` — anything in
    /// [`Self::types`]. These are the only structs/enums the
    /// per-item emitter walks; bodies of undeclared types are
    /// skipped.
    fn declared_types(&self) -> std::collections::HashSet<TypeKey> {
        self.types.keys().cloned().collect()
    }

    /// Emit the `OwnedObject<T>` borrow wrapper used by
    /// [`Self::opaque_handle_input`] into the destination file.
    /// The struct is referenced by an unqualified `OwnedObject` from
    /// the same generated file, so no `use` paths leak into the host
    /// crate's source tree.
    fn prerequisites(&self, registry: &Registry<KotlinMeta>) -> Vec<syn::Item> {
        // `__JniErr` is the **framework** error type alias — always the
        // pre-registered `JniBindingError`, never a user-declared
        // application exception. Built-in converter bodies compose
        // their `?` failures into this type via its `From<String>`
        // impl, so a built-in decode failure surfaces as
        // `JniBindingError` on the JVM. Throwing converters
        // (closures returning `Some(parse_quote!(<full path>))` in the middle slot of
        // `input_wrapper` / `output_wrapper`) instead emit functions
        // typed `Result<…, X>` — they bypass `__JniErr` entirely so no
        // cross-type bridge between the framework error and a domain
        // error is needed (the orphan rule forbids one).
        let error_type = &self.framework_exception().rust_type;
        let alias: syn::Item = syn::parse_quote!(
            #[allow(dead_code)]
            pub(crate) type __JniErr = #error_type;
        );
        let mut items = vec![alias];
        items.extend(owned_object_prerequisite_items());
        // Throw fns — one `pub(crate) fn throw_<short>(env, &err)` per
        // registered throwable class (via `.throwable()`). Emitted as prerequisites
        // (above the converters) so the wrappers below can reference
        // them by bare name; the binding crate references them as
        // `<include_module>::throw_<short>` from outside the file.
        items.extend(
            self.exceptions
                .iter()
                .map(|exc| build_throw_fn_item(self, registry, exc)),
        );
        // Handle destructors — one `extern "C" freePtr<suffix>` per
        // non-suppressed opaque handle (the Rust half of the typed-handle
        // `free()` pair the Kotlin emitter generates).
        items.extend(build_handle_destructor_items(self, registry));
        items
    }

    fn post_process_item(&self, item: &mut syn::Item) {
        self.qualify_item(item);
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
        // Structured-config overrides first (opaque handles, then user-
        // registered rank-0 wrappers, then built-ins).
        let key = TypeKey::from_type(ty);
        if let Some(cfg) = self.types.get(&key) {
            if cfg.opaque.is_some() {
                return Some(self.opaque_handle_input(ty));
            }
        }
        // `kotlin_enum`-declared enums: jint wire, `TryFrom<i32>` decode.
        // Registered before the user-wrapper lookup so a stray
        // `input_wrapper` registration on the same key would have to be
        // intentional. The rank-0 enum arm produces a terminal converter
        // (jint → Rust enum) with the configured Kotlin FQN in metadata.
        if let Some(cfg) = self.types.get(&key) {
            if cfg.enum_cfg.is_some() {
                if let Some(name) = bare_path_ident(ty) {
                    if let Some((e, _)) = registry.enums.get(&name) {
                        let (wire, body) = enum_input_body(self, e);
                        let niches = default_niches_for_wire(&wire);
                        let kotlin_name = cfg.kotlin_name.clone();
                        return Some(ConverterImpl {
                            pre_stages: vec![],
                            function: self.build_input_fn(ty, &wire, &body, None),
                            destination: wire,
                            niches,
                            metadata: self.framework_meta(kotlin_name),
                        });
                    }
                }
            }
        }
        if let Some(conv) = self.lookup_input(ty, &[], registry) {
            return Some(conv);
        }
        if let Some((wire, body)) = primitive_input(ty) {
            let niches = default_niches_for_wire(&wire);
            let kotlin_name = crate::jni::jni_kotlin_ext::kotlin_for_wire(&wire);
            return Some(ConverterImpl {
                pre_stages: vec![],
                function: self.build_input_fn(ty, &wire, &body, None),
                destination: wire,
                niches,
                metadata: self.framework_meta(kotlin_name),
            });
        }
        if let Some(name) = bare_path_ident(ty) {
            if let Some((s, _)) = registry.structs.get(&name) {
                let (wire, body) = struct_input_body(self, s, registry)?;
                let niches = default_niches_for_wire(&wire);
                // Auto-generated struct: the value-context Kotlin name is
                // whatever the user pinned via `kotlin_data_class`. If
                // they didn't, leave `kotlin_name = None` — emitter
                // surfaces this as a build-time hard error.
                let kotlin_name = self.types.get(&key).and_then(|c| c.kotlin_name.clone());
                return Some(ConverterImpl {
                    pre_stages: vec![],
                    function: self.build_input_fn(ty, &wire, &body, None),
                    destination: wire,
                    niches,
                    metadata: self.framework_meta(kotlin_name),
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
        if let Some(conv) = self.lookup_input(pat, &[t1.clone()], registry) {
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
            // `&T` shares T's converter function verbatim, so it inherits
            // T's throws behaviour (whatever exception T's converter is
            // bound to). Copy the inner's throws metadata.
            // A borrowed handle is still opaque (param classification
            // needs to see it), but the holder doesn't own it — mark
            // `owned: false` so `close()` emission skips it.
            let handle = inner.metadata.handle.clone().map(|h| HandleInfo {
                owned: false,
                ..h
            });
            return Some(ConverterImpl {
                destination: inner.destination.clone(),
                function: inner.function.clone(),
                pre_stages: vec![],
                niches: inner.niches.clone(),
                metadata: KotlinMeta {
                    kotlin_name,
                    throws: inner.metadata.throws.clone(),
                    throws_action: inner.metadata.throws_action.clone(),
                    value_rust_key: None,
                    handle,
                },
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
            // Fold a Nullable layer over the inner handle (if any), so an
            // `Option<Handle>` field/param carries the full close strategy.
            let handle = registry
                .input_entry(t1)
                .and_then(|e| e.metadata.handle.clone())
                .map(|h| HandleInfo {
                    strategy: CloseStrategy::Nullable(Box::new(h.strategy)),
                    ..h
                });
            return Some(ConverterImpl {
                pre_stages: vec![],
                function: self.build_input_fn(&outer_ty, &wire, &body, None),
                destination: wire,
                niches,
                metadata: KotlinMeta {
                    handle,
                    ..self.framework_meta(kotlin_name)
                },
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
        // Kotlin sees `impl Fn(...)` as the matching mangled
        // fun-interface (zenoh-jni: `JNIOn<Args>`). Use the
        // registration-stamped FQN when set; fall back to the
        // auto-derived name.
        let outer_key = TypeKey::from_type(&outer_ty);
        let kotlin_name = self
            .types
            .get(&outer_key)
            .and_then(|c| c.callback_kotlin_fqn.clone())
            .or_else(|| Some(self.auto_callback_fqn(args)));
        Some(ConverterImpl {
            pre_stages: vec![],
            function: self.build_input_fn(&outer_ty, &wire, &body, None),
            destination: wire,
            niches,
            metadata: self.framework_meta(kotlin_name),
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
        self.lookup_input(pat, &[t1.clone(), t2.clone()], registry)
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
        self.lookup_input(pat, &[t1.clone(), t2.clone(), t3.clone()], registry)
    }

    // ── Output converters ────────────────────────────────────────────

    fn on_output_type_rank_0(
        &self,
        ty: &syn::Type,
        registry: &Registry<KotlinMeta>,
    ) -> Option<ConverterImpl<KotlinMeta>> {
        // Structured-config overrides first (opaque handles, then the
        // unified user-registered wrapper table, then built-ins).
        let key = TypeKey::from_type(ty);
        if let Some(cfg) = self.types.get(&key) {
            if cfg.opaque.is_some() {
                return Some(self.opaque_handle_output(ty));
            }
        }
        // `kotlin_enum`-declared enums: jint wire, `as jni::sys::jint`
        // encode. Symmetric to the input arm above; relies on
        // `#[repr(i32)]` (or any repr that supports the cast) on the
        // declared enum so the discriminant value round-trips identically.
        if let Some(cfg) = self.types.get(&key) {
            if cfg.enum_cfg.is_some() {
                if let Some(name) = bare_path_ident(ty) {
                    if let Some((e, _)) = registry.enums.get(&name) {
                        let (wire, body) = enum_output_body(self, e);
                        let niches = default_niches_for_wire(&wire);
                        let kotlin_name = cfg.kotlin_name.clone();
                        return Some(ConverterImpl {
                            pre_stages: vec![],
                            function: self.build_output_fn(ty, &wire, &body, None),
                            destination: wire,
                            niches,
                            metadata: self.framework_meta(kotlin_name),
                        });
                    }
                }
            }
        }
        if let Some(conv) = self.lookup_output(ty, &[], registry) {
            return Some(conv);
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
                function: self.build_output_fn(ty, &wire, &body, None),
                destination: wire,
                pre_stages: vec![],
                niches: Niches::empty(),
                metadata: KotlinMeta::default(),
            });
        }
        if let Some((wire, body)) = primitive_output(ty) {
            let niches = default_niches_for_wire(&wire);
            let kotlin_name = crate::jni::jni_kotlin_ext::kotlin_for_wire(&wire);
            return Some(ConverterImpl {
                pre_stages: vec![],
                function: self.build_output_fn(ty, &wire, &body, None),
                destination: wire,
                niches,
                metadata: self.framework_meta(kotlin_name),
            });
        }
        if let Some(name) = bare_path_ident(ty) {
            if let Some((s, _)) = registry.structs.get(&name) {
                let (wire, body) = struct_output_body(self, s, registry)?;
                let niches = default_niches_for_wire(&wire);
                let kotlin_name = self.types.get(&key).and_then(|c| c.kotlin_name.clone());
                return Some(ConverterImpl {
                    pre_stages: vec![],
                    function: self.build_output_fn(ty, &wire, &body, None),
                    destination: wire,
                    niches,
                    metadata: self.framework_meta(kotlin_name),
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
        if let Some(conv) = self.lookup_output(pat, &[t1.clone()], registry) {
            return Some(conv);
        }
        // `Result<_, _>` is handled as a built-in rank-2 wrapper registered
        // in `JniExt::new`. Bindings just declare the Err type via
        // `.throwable()`. Per-error overrides are possible by registering a
        // more specific rank-1 `output_wrapper(Result<_, ConcreteErr>, …)`
        // — rank-1 fires before rank-2 in resolve and short-circuits here.
        if pat_match(pat, "Option < _ >") {
            let outer_ty: syn::Type = syn::parse_quote!(Option<#t1>);
            let (wire, body, niches) = option_output(t1, registry)?;
            let inherited = registry
                .output_entry(t1)
                .and_then(|e| e.metadata.kotlin_name.clone());
            let kotlin_name = self.override_kotlin_name(&outer_ty, inherited);
            // Fold a Nullable layer over the inner handle (if any), so an
            // `Option<Handle>` output carries the full close strategy.
            let handle = registry
                .output_entry(t1)
                .and_then(|e| e.metadata.handle.clone())
                .map(|h| HandleInfo {
                    strategy: CloseStrategy::Nullable(Box::new(h.strategy)),
                    ..h
                });
            return Some(ConverterImpl {
                pre_stages: vec![],
                function: self.build_output_fn(&outer_ty, &wire, &body, None),
                destination: wire,
                niches,
                metadata: KotlinMeta {
                    handle,
                    ..self.framework_meta(kotlin_name)
                },
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
        self.lookup_output(pat, &[t1.clone(), t2.clone()], registry)
    }

    fn on_output_type_rank_3(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        t2: &syn::Type,
        t3: &syn::Type,
        registry: &Registry<KotlinMeta>,
    ) -> Option<ConverterImpl<KotlinMeta>> {
        self.lookup_output(pat, &[t1.clone(), t2.clone(), t3.clone()], registry)
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

    // Throw fn for framework binding failures — the fallback when a
    // converter carries no explicit bound exception (non-throwing).
    let framework_throw = exception_throw_path(ext.framework_exception());

    let mut wire_params: Vec<TokenStream> = Vec::new();
    // Each entry is a per-input decode statement. Fallible decodes are
    // `match`-arms that dispatch to the input converter's own throw fn
    // on `Err` and `return <sentinel>;` — so a malformed `Encoding`
    // JObject raises `JniBindingError`, while a throwing input wrapper
    // raises whatever exception it bound via `Some(parse_quote!(...))` in the closure.
    let mut prelude: Vec<TokenStream> = Vec::new();
    let mut call_args: Vec<TokenStream> = Vec::new();

    // Output is resolved first so the per-input `match`-arms can splice
    // the function's sentinel into their early-`return` path.
    let return_ty: syn::Type = match &f.sig.output {
        syn::ReturnType::Default => syn::parse_quote!(()),
        syn::ReturnType::Type(_, ty) => (**ty).clone(),
    };
    let output_entry = registry.output_entry(&return_ty).unwrap_or_else(|| {
        panic!(
            "JniExt::on_function: return type `{}` of `{}` has no registered output \
             converter — register one via `JniExt::output_wrapper(pat, |…| Some((ty, exc, body)))` \
             (exc = `None` for non-throwing, `Some(parse_quote!(<full path>))` \
              to bind a domain exception)",
            TypeKey::from_type(&return_ty),
            original_ident,
        )
    });
    let wire_return_ty = output_entry.destination.clone();
    let conv_out = output_entry.function.sig.ident.clone();
    let wire_return_lt = annotate_jobject_with_lifetime(&wire_return_ty, "a");
    let wire_return = wire_return_lt.to_token_stream();
    let on_err: TokenStream = sentinel_for_wire(&wire_return_ty);

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
        // Each input converter carries the throw fn for its failures —
        // framework `throw_JniBindingError` by default, or a custom one
        // bound via `Some(parse_quote!(<full path>))` in the input
        // wrapper's closure.
        let input_throw = entry
            .metadata
            .throws_action
            .clone()
            .unwrap_or_else(|| framework_throw.clone());
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
        // work too. This decode is infallible — no `match` needed.
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
        // Input wrapper takes wires by ref except for raw pointers. The
        // converter returns `Result<T, __JniErr>`; on `Err` we throw via
        // this input's own throw fn and bail with the function sentinel.
        let decode_call = if matches!(wire, syn::Type::Ptr(_)) {
            quote!(#conv(&mut env, #wire_ident))
        } else {
            quote!(#conv(&mut env, &#wire_ident))
        };
        // Stage 0: wire-facing function. Pre_stages then run in REVERSE
        // (rust-side last). Even with no pre_stages this collapses to a
        // single `let #arg_ident = match decode_call { ... }`, byte-
        // identical to the pre-chain emission.
        if entry.pre_stages.is_empty() {
            prelude.push(quote!(
                let #arg_ident = match #decode_call {
                    ::core::result::Result::Ok(__v) => __v,
                    ::core::result::Result::Err(__e) => {
                        #input_throw(&mut env, &__e);
                        return #on_err;
                    }
                };
            ));
        } else {
            // Multi-stage: introduce a temporary for the function's
            // result, then thread each pre_stage in reverse onto it.
            let stage0_ident = format_ident!("__{}_s0", arg_ident);
            prelude.push(quote!(
                let #stage0_ident = match #decode_call {
                    ::core::result::Result::Ok(__v) => __v,
                    ::core::result::Result::Err(__e) => {
                        #input_throw(&mut env, &__e);
                        return #on_err;
                    }
                };
            ));
            let mut prev = stage0_ident;
            // pre_stages[0] is closest to rust → iterated last; walk
            // back from the function-adjacent end.
            let n = entry.pre_stages.len();
            for (idx, stage) in entry.pre_stages.iter().enumerate().rev() {
                let stage_fn = &stage.function.sig.ident;
                let stage_throw = &stage.throws_action;
                let is_last = idx == 0;
                let out_ident = if is_last {
                    arg_ident.clone()
                } else {
                    format_ident!("__{}_s{}", arg_ident, n - idx)
                };
                prelude.push(quote!(
                    let #out_ident = match #stage_fn(&mut env, #prev) {
                        ::core::result::Result::Ok(__v) => __v,
                        ::core::result::Result::Err(__e) => {
                            #stage_throw(&mut env, &__e);
                            return #on_err;
                        }
                    };
                ));
                prev = out_ident;
            }
        }
        if matches!(arg_ty, syn::Type::Reference(_)) {
            call_args.push(quote!(&#arg_ident));
        } else {
            call_args.push(quote!(#arg_ident));
        }
    }

    let call_expr = quote!(#source_module::#original_ident(#(#call_args),*));

    // Output phase. Every output converter now returns
    // `Result<wire, <err_type>>` — the bare-wire shape is gone.
    // Unwrap and dispatch to the converter's `throws_action`
    // (framework `throw_JniBindingError` for plain wrappers, a domain
    // throw fn for throws-marked wrappers).
    //
    // Pre_stages (rust-side throw stages) run in forward order BEFORE
    // the wire-facing function: rust → pre_stages[0] → … →
    // pre_stages[N-1] → function → wire. Each stage's match-throw
    // dispatches to its own configured exception class.
    let out_throw = output_entry
        .metadata
        .throws_action
        .clone()
        .unwrap_or_else(|| framework_throw.clone());
    let mut output_phase: TokenStream = quote! { let __out = #call_expr; };
    let mut prev_out: TokenStream = quote!(__out);
    for (i, stage) in output_entry.pre_stages.iter().enumerate() {
        let stage_fn = &stage.function.sig.ident;
        let stage_throw = &stage.throws_action;
        let next_ident = format_ident!("__out_s{}", i);
        output_phase.extend(quote! {
            let #next_ident = match #stage_fn(&mut env, #prev_out) {
                ::core::result::Result::Ok(__v) => __v,
                ::core::result::Result::Err(__e) => {
                    #stage_throw(&mut env, &__e);
                    return #on_err;
                }
            };
        });
        prev_out = quote!(#next_ident);
    }
    output_phase.extend(quote! {
        match #conv_out(&mut env, #prev_out) {
            ::core::result::Result::Ok(__w) => __w,
            ::core::result::Result::Err(__e) => {
                #out_throw(&mut env, &__e);
                #on_err
            }
        }
    });

    quote! {
        #[no_mangle]
        #[allow(non_snake_case, unused_mut, unused_variables, dead_code)]
        pub unsafe extern "C" fn #wrapper_ident<'a>(
            mut env: jni::JNIEnv<'a>,
            _class: jni::objects::JClass<'a>,
            #(#wire_params),*
        ) -> #wire_return {
            #(#prelude)*
            #output_phase
        }
    }
}

/// Last-segment ident of a `TypeKey` — e.g. `"Publisher<'static>"` →
/// `"Publisher"`, `"AdvancedSubscriber<()>"` → `"AdvancedSubscriber"`. Used by
/// the structured builders ([`JniExt::kotlin_ptr_class`],
/// [`JniExt::kotlin_data_class`]) to derive a default Kotlin class name from
/// the Rust type-key. Panics for non-path types (e.g. closures, references) —
/// the per-kind `kotlin_*_name_mangle` closures see only path-shaped
/// shorts. For verbatim Kotlin expressions on non-path types, chain
/// [`JniExt::with_kotlin_type`] after the structured builder.
fn rust_short_name(key: &TypeKey) -> String {
    rust_short_name_opt(key).unwrap_or_else(|| {
        panic!(
            "rust_short_name: cannot derive Kotlin name from type-key `{}` — \
             only path-shaped types are supported here; use \
             `with_kotlin_type(\"<verbatim>\")` to set the name explicitly",
            key.as_str()
        )
    })
}

/// Fallible variant of [`rust_short_name`] — returns `None` for
/// non-path types instead of panicking. Used by
/// [`JniExt::note_wrapper_registration`] which is called for rank-0
/// wrapper patterns including non-path shapes like `()` where there
/// is no Kotlin short name to derive.
fn rust_short_name_opt(key: &TypeKey) -> Option<String> {
    let ty = key.to_type();
    if let syn::Type::Path(tp) = &ty {
        if let Some(last) = tp.path.segments.last() {
            return Some(last.ident.to_string());
        }
    }
    None
}

fn type_last_ident(ty: &syn::Type) -> Option<syn::Ident> {
    if let syn::Type::Path(tp) = ty {
        if let Some(last) = tp.path.segments.last() {
            return Some(last.ident.clone());
        }
    }
    None
}

/// `VisitMut` that prefixes every bare single-segment `Type::Path` whose
/// ident lives in `source_names` with `source_module`. Walks the full
/// AST — function signatures, generic args, type ascriptions, casts,
/// turbofish — so any emitted item passes through one universal pass
/// instead of each emit site having to remember to qualify.
struct QualifyEmittedTypes<'a> {
    source_module: &'a syn::Path,
    source_names: &'a std::collections::HashSet<String>,
}

impl<'a> syn::visit_mut::VisitMut for QualifyEmittedTypes<'a> {
    fn visit_type_path_mut(&mut self, tp: &mut syn::TypePath) {
        if tp.qself.is_none()
            && tp.path.leading_colon.is_none()
            && tp.path.segments.len() == 1
        {
            let ident = tp.path.segments[0].ident.to_string();
            if self.source_names.contains(&ident) {
                let mut qualified = self.source_module.clone();
                qualified.segments.push(tp.path.segments[0].clone());
                tp.path = qualified;
            }
        }
        syn::visit_mut::visit_type_path_mut(self, tp);
    }
}

fn mangle_jni_name(ext: &JniExt, ident: &syn::Ident) -> syn::Ident {
    let camel = snake_to_camel(&ident.to_string());
    let mangled = ext.mangle_fun(&camel);
    let mut name = ext.jni_class_path.clone();
    name.push('_');
    name.push_str(&mangled);
    syn::Ident::new(&name, Span::call_site())
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
        "i32" => (
            syn::parse_quote!(jni::sys::jint),
            syn::parse_quote!(*v),
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
        "i32" => (
            syn::parse_quote!(jni::sys::jint),
            syn::parse_quote!(v as jni::sys::jint),
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
        let returns_owned_object = converter_returns_owned_object(&inner_entry.function.sig.output);
        let body: syn::Expr = if returns_owned_object {
            // Borrow semantics: the Java side still owns the boxed value
            // (its `close()` will free the original Box later via the typed
            // handle's `freePtr`). Cloning the inner T keeps the pointer
            // live across this call — using `Box::from_raw` here would
            // consume the box, leaving the Java slot dangling and causing
            // a double-free the next time the same data-class instance is
            // decoded. Requires `T: Clone`.
            syn::parse_quote!({
                if #pred {
                    None
                } else {
                    Some(unsafe { OwnedObject::from_raw(*v as *const #t1).clone() })
                }
            })
        } else {
            syn::parse_quote!({
                if #pred { None } else { Some(#inner_conv(env, v)?) }
            })
        };
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
    let name = derive_callback_name(args);

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

    let name_lit = syn::LitStr::new(&name, Span::call_site());
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
                    .map_err(|e| <__JniErr as ::core::convert::From<String>>::from(format!("Attach thread for {}: {}", #name_lit, e)))?;
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
            .map_err(|e| tracing::error!("{} callback error: {e}", #name_lit));
        })
    });

    // The destination type for an `impl Fn(args)` parameter is JObject (the
    // Kotlin callback object). We return Box<dyn Fn(args) + Send + Sync>
    // wrapped in a generic so it satisfies the impl-trait param type.
    // Actually the SOURCE (rust) type IS `impl Fn(args) + Send + Sync + 'static`,
    // so the wrapper's return type is that. Box<dyn Fn> coerces.
    Some((syn::parse_quote!(jni::objects::JObject), body))
}

fn derive_callback_name(args: &[syn::Type]) -> String {
    let mut s = String::new();
    for a in args {
        s.push_str(&type_short_ident(a));
    }
    s.push_str("Callback");
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

/// Resolve the typed-handle Kotlin FQN for a handle-bearing struct field
/// and assert its folded strategy is one the struct encode/decode bridge
/// supports. Today only scalar handle slots (`Direct`, optionally wrapped
/// in `Nullable`) are encodable as a single `L<FQN>;` ctor arg; a
/// collection layer (`Iterable`, i.e. `Vec<Handle>`) would need array
/// codegen and is a loud build-time error until implemented.
fn handle_field_fqn(ext: &JniExt, h: &HandleInfo) -> String {
    fn assert_scalar(s: &CloseStrategy) {
        match s {
            CloseStrategy::Direct => {}
            CloseStrategy::Nullable(inner) => assert_scalar(inner),
            CloseStrategy::Iterable(_) => panic!(
                "struct handle field: collection (Vec<Handle>) layers are not yet \
                 supported by the struct encode/decode bridge — add array codegen \
                 to struct_output_body/struct_input_body to lift this guard"
            ),
        }
    }
    assert_scalar(&h.strategy);
    ext.kotlin_type_fqns
        .iter()
        .find(|(k, _)| k == &h.leaf_key)
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| {
            panic!(
                "struct handle field: leaf `{}` has no Kotlin FQN registered \
                 (kotlin_ptr_class)",
                h.leaf_key
            )
        })
}

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

        // Mirror of the struct_output_body bridge: when the field is a
        // closeable native handle (folded `HandleInfo` on its converter
        // metadata), read the JNINativeHandle object from the JVM slot,
        // `peek()` the raw jlong, then run the per-field input converter
        // (still jlong-keyed). Null handle ⇒ jlong = 0 ⇒ `None` via the
        // niche-path. Detection now flows from the type-unfolding metadata,
        // not a syntactic `Option<T>` peel.
        if let Some(h) = &field_entry.metadata.handle {
            let java_path = handle_field_fqn(ext, h).replace('.', "/");
            let sig = format!("L{};", java_path);
            let tmp_ident = format_ident!("__{}_jobj", fname_ident);
            field_preludes.push(quote! {
                let #tmp_ident: jni::objects::JObject = env.get_field(v, #camel, #sig)
                    .and_then(|val| val.l())
                    .map_err(|e| <__JniErr as ::core::convert::From<String>>::from(format!(#err_prefix, e)))?;
                let #raw_ident: jni::sys::jlong = if #tmp_ident.is_null() {
                    0
                } else {
                    env.call_method(&#tmp_ident, "peek", "()J", &[])
                        .and_then(|val| val.j())
                        .map_err(|e| <__JniErr as ::core::convert::From<String>>::from(format!(#err_prefix, e)))?
                };
                let #fname_ident = #field_conv(env, &#raw_ident)?;
            });
            field_init.push(quote!(#fname_ident));
            continue;
        }

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
    // Prefer the registered Kotlin FQN (`io.zenoh.jni.JniSample`) so the
    // mangle closure flows through; fall back to the bare struct ident
    // qualified with the package when no `kotlin_data_class` /
    // `kotlin_ptr_class` declaration exists for this Rust type.
    let struct_ident = &s.ident;
    let struct_ty: syn::Type = syn::parse_quote!(#struct_ident);
    let registered_fqn = ext
        .types
        .get(&TypeKey::from_type(&struct_ty))
        .and_then(|cfg| cfg.kotlin_name.clone());
    let java_class_name = if let Some(fqn) = registered_fqn {
        fqn.replace('.', "/")
    } else if ext.java_class_prefix.is_empty() {
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

        // Closeable native-handle fields are bridged: the per-field output
        // converter still produces `jlong` (sentinel 0L for None); we wrap
        // that into the typed-handle Kotlin class so the data-class field
        // type matches. Detection flows from the folded `HandleInfo` the
        // type-unfolding mechanism propagated onto this field's metadata.
        // JVM ctor slot is `L<typed-FQN>;`, not `J`.
        if let Some(h) = &field_entry.metadata.handle {
            let java_path = handle_field_fqn(ext, h).replace('.', "/");
            let ctor_slot = format!("L{};", java_path);
            ctor_sig.push_str(&ctor_slot);
            let java_path_lit = syn::LitStr::new(&java_path, Span::call_site());
            field_preludes.push(quote! {
                let #encoded_obj_ident: jni::objects::JObject<'a> = if #encoded_ident == 0 {
                    jni::objects::JObject::null()
                } else {
                    env
                        .new_object(#java_path_lit, "(J)V", &[jni::objects::JValue::from(#encoded_ident)])
                        .map_err(|e| <__JniErr as ::core::convert::From<String>>::from(format!("wrap typed handle {}: {}", #java_path_lit, e)))?
                };
            });
            ctor_args.push(quote!(jni::objects::JValue::Object(&#encoded_obj_ident)));
            continue;
        }

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
// Enum rank-0 bodies
// ──────────────────────────────────────────────────────────────────────

/// `jint → Rust enum` decoder body for a `kotlin_enum`-declared enum.
/// Wire is `jni::sys::jint`. The framework builds the decode `match`
/// directly from the enum's own discriminants — no `TryFrom<i32>` impl
/// is required on the flat enum (the enum declaration is the single
/// source of truth for the int↔variant mapping, shared with the Kotlin
/// `value(N)` constants via [`enum_discriminant_values`]). An unknown
/// discriminant surfaces as the framework `__JniErr`.
///
/// The arms use the bare ident — same shape as the wrapper function's
/// `v: <ident>` signature — so binding crates can pick whichever
/// upstream type a bare `<ident>` resolves to in their include-site
/// `use` statements. Pairs with output body below.
fn enum_input_body(_ext: &JniExt, e: &syn::ItemEnum) -> (syn::Type, syn::Expr) {
    assert_only_unit_variants(e);
    let ident = &e.ident;
    let ident_name = ident.to_string();
    let arms = crate::util::enum_discriminant_values(e).into_iter().map(|(variant, value)| {
        let lit = proc_macro2::Literal::i64_unsuffixed(value);
        quote! { #lit => #ident::#variant, }
    });
    let body: syn::Expr = syn::parse_quote!({
        match *v as i64 {
            #(#arms)*
            other => {
                return ::core::result::Result::Err(
                    <__JniErr as ::core::convert::From<String>>::from(
                        format!("invalid {} discriminant: {}", #ident_name, other)
                    )
                );
            }
        }
    });
    (syn::parse_quote!(jni::sys::jint), body)
}

/// `Rust enum → jint` encoder body for a `kotlin_enum`-declared enum.
/// Wire is `jni::sys::jint`. Relies on the declared enum's repr
/// supporting an `as` cast (i.e. C-like enum, no fields); the
/// [`assert_only_unit_variants`] check below catches violations
/// upstream of the cast. The body works without naming the enum type
/// at all — `v` is already typed via the wrapper signature, so the
/// `as` cast picks up the right type by inference.
fn enum_output_body(_ext: &JniExt, e: &syn::ItemEnum) -> (syn::Type, syn::Expr) {
    assert_only_unit_variants(e);
    let body: syn::Expr = syn::parse_quote!({ v as jni::sys::jint });
    (syn::parse_quote!(jni::sys::jint), body)
}

/// Hard error on any enum that's not C-like (unit variants only).
/// `kotlin_enum`'s discriminant-keyed Kotlin emission and `as jint`
/// encode both depend on unit variants — bail loudly at build time
/// rather than emitting wrong code.
fn assert_only_unit_variants(e: &syn::ItemEnum) {
    for variant in &e.variants {
        if !matches!(variant.fields, syn::Fields::Unit) {
            panic!(
                "kotlin_enum only supports C-like enums (unit variants), \
                 but `{}::{}` has fields",
                e.ident, variant.ident
            );
        }
    }
}

// ──────────────────────────────────────────────────────────────────────
// JNI primitive (un)boxing helpers
// ──────────────────────────────────────────────────────────────────────

pub(crate) fn is_jni_primitive(ty: &syn::Type) -> bool {
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
/// case: `impl Fn(...)` keeps the legacy `process_kotlin_<Name>_callback`
/// name so existing hand-written call sites continue to resolve. With
/// the current [`derive_callback_name`] algorithm `<Name>` is
/// concatenated arg shorts + `"Callback"` (e.g. `process_kotlin_SampleCallback_callback`).
fn input_name(rust: &syn::Type, wire: &syn::Type) -> syn::Ident {
    if let Some(args) = extract_fn_trait_args(rust) {
        let name = derive_callback_name(&args);
        let s = format!("process_kotlin_{}_callback", name);
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
            pre_stages: vec![],
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
