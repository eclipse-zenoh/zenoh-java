//! `PrebindgenExt` — the single extension point for the new pipeline.
//!
//! One method per `#[prebindgen]` item kind (`on_function`, `on_struct`,
//! `on_enum`, `on_const`) returning the wrapper Rust tokens to emit, plus a
//! family of converter methods split by direction and rank:
//!
//! * Input  (wire → rust): `on_input_type_rank_0..3`
//! * Output (rust → wire): `on_output_type_rank_0..3`
//!
//! Each converter method returns `Some(ConverterImpl)` if the ext handles
//! the type, or `None` to fall through to higher-rank wildcard attempts (and
//! ultimately to an "unresolved required type" error if the resolver can't
//! fill the cell).
//!
//! `ConverterImpl::function` is the **complete** Rust function for the
//! converter — signature, body, attributes, lifetimes. The plugin owns
//! 100% of the shape. Other code that wants to call this converter reads
//! the name from `function.sig.ident`; the wire form from `destination`.

use proc_macro2::TokenStream;

use crate::core::niches::Niches;
use crate::core::registry::Registry;

/// Result of resolving one converter — the wire (destination) type the rest
/// of the registry sees, plus the complete generated function.
///
/// Invariant: `function.sig.ident` MUST be a deterministic function of the
/// `(rust_type, destination)` pair so that callers of this converter — both
/// other generated converters in the same plugin and any hand-written code
/// that knows the convention — can compute or look up the name.
#[derive(Clone)]
pub struct ConverterImpl<M = ()> {
    /// Wire/destination type. Other converters that ask "what's the wire
    /// form of this rust type?" read this. The actual function may return
    /// a wrapped form (e.g. `ZResult<destination>`) — that is the plugin's
    /// internal calling convention; `destination` is the value the wire
    /// carries on success.
    pub destination: syn::Type,
    /// Complete function definition. The plugin owns the parameter list,
    /// return type, `unsafe`/`pub` modifiers, lifetime parameters, and any
    /// attribute annotations.
    pub function: syn::ItemFn,
    /// Bit-patterns the wire type can represent but this converter never
    /// produces (output) and rejects (input). Wrapper handlers like
    /// `Option<_>` consume one slot for their own discriminant and
    /// re-export the rest — see [`Niches`] for the cascade model.
    /// Default is empty (no niche optimisation).
    pub niches: Niches,
    /// Language-specific extras carried alongside the converter. Filled by
    /// the same handler that produces `destination` / `function` /
    /// `niches`, copied through into `TypeEntry::metadata` by the resolver,
    /// and read by language-side emitters. Set this where you build the
    /// converter, not in a side channel.
    pub metadata: M,
}

/// How a single `impl Into<target>` source arm consumes the Java-side
/// value when the source maps to an opaque-handle Rust type (i.e. the
/// source's registered input decoder returns `OwnedObject<T>`). The
/// mode is a no-op for non-opaque sources (they have no `Box` slot to
/// manage).
///
/// Used by [`IntoSource`] to drive
/// [`PrebindgenExt::dispatch_into_input`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntoSourceMode {
    /// Borrow: opaque sources decode via
    /// `OwnedObject::from_raw(...).clone()` (Java's `Box` slot stays
    /// live across the call; requires `T: Clone`).
    Borrow,
    /// Consume: opaque sources decode via
    /// `*Box::from_raw(ptr as *mut T)` (Java's `Box` slot is taken;
    /// caller's typed handle is invalidated by the call). No
    /// `T: Clone` bound.
    Consume,
}

/// One source arm in the dispatcher for an
/// `impl Into<target> + Send + 'static` parameter — the Rust source
/// type plus the borrow/consume mode that determines how the opaque
/// `Box` slot is treated when the source is an opaque-handle type.
///
/// Order in [`PrebindgenExt::into_sources`]'s returned vector
/// determines the runtime dispatch order in the emitted converter.
#[derive(Clone)]
pub struct IntoSource {
    /// Rust source type the arm decodes from before
    /// `TryInto::<target>::try_into` runs.
    pub source_type: syn::Type,
    /// Borrow vs. Consume — relevant only when `source_type` is an
    /// opaque-handle type (input decoder returns `OwnedObject<T>`).
    pub mode: IntoSourceMode,
}

impl IntoSource {
    /// Borrow-mode arm — opaque sources keep the Java handle live
    /// (`OwnedObject::from_raw(...).clone()`); non-opaque sources are
    /// unaffected. Equivalent to today's universal behavior.
    pub fn borrow(ty: syn::Type) -> Self {
        Self {
            source_type: ty,
            mode: IntoSourceMode::Borrow,
        }
    }

    /// Consume-mode arm — opaque sources take ownership of the Java
    /// slot (`*Box::from_raw(ptr as *mut T)`), invalidating the
    /// caller's typed handle. Non-opaque sources are unaffected.
    pub fn consume(ty: syn::Type) -> Self {
        Self {
            source_type: ty,
            mode: IntoSourceMode::Consume,
        }
    }
}

/// Implemented by destination-language back-ends (e.g. JNI). The resolver
/// drives this trait to fill `Registry::input_types` / `output_types`
/// entries; the file emitter calls `on_function` / `on_struct` / `on_enum` /
/// `on_const` to produce per-item wrapper code.
///
/// Back-ends pick a [`Self::Metadata`] type to carry language-specific
/// extras (Kotlin names, C header names, …) end-to-end through the
/// pipeline — set in each `ConverterImpl::metadata`, propagated by the
/// resolver into `TypeEntry::metadata`, and read directly by emitter
/// code. Back-ends that don't need any extras leave it at the default
/// `()`.
pub trait PrebindgenExt {
    /// Language-specific extras every resolved converter carries. The
    /// resolver copies this from each `ConverterImpl` it accepts into
    /// the matching `TypeEntry`, so emitter code reads metadata off
    /// the registry rather than through a parallel side channel.
    type Metadata: Clone + Default;

    /// Rust items the plugin's emitted converters depend on (helper
    /// structs, type aliases, runtime-support code). Emitted at the top
    /// of the destination file, before all auto-generated converters.
    ///
    /// Default: none. Wrapper exts that compose a base ext should
    /// forward to / extend the base's `prerequisites()`.
    fn prerequisites(&self) -> Vec<syn::Item> {
        Vec::new()
    }

    // ── Item methods ───────────────────────────────────────────────

    /// Wrap a `#[prebindgen]` fn into the destination-language wrapper
    /// (e.g. JNI `extern "C"` fn).
    fn on_function(&self, f: &syn::ItemFn, registry: &Registry<Self::Metadata>) -> TokenStream;

    /// Per-struct emission. Typically empty for languages that get
    /// everything they need from auto-generated converters.
    fn on_struct(&self, s: &syn::ItemStruct, registry: &Registry<Self::Metadata>) -> TokenStream;

    /// Per-enum emission.
    fn on_enum(&self, e: &syn::ItemEnum, registry: &Registry<Self::Metadata>) -> TokenStream;

    /// Per-const emission. Default: pass-through.
    fn on_const(&self, c: &syn::ItemConst, _registry: &Registry<Self::Metadata>) -> TokenStream {
        use quote::ToTokens;
        c.to_token_stream()
    }

    // ── Input direction (wire → rust) ──────────────────────────────

    /// Whole-type input converter. Returns `Some(ConverterImpl)` if the
    /// ext handles `ty`.
    fn on_input_type_rank_0(
        &self,
        ty: &syn::Type,
        registry: &Registry<Self::Metadata>,
    ) -> Option<ConverterImpl<Self::Metadata>>;

    /// Single-wildcard input pattern. `pat` contains one `_`; `t1` is the
    /// type the wildcard slot held in the original.
    fn on_input_type_rank_1(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        registry: &Registry<Self::Metadata>,
    ) -> Option<ConverterImpl<Self::Metadata>>;

    fn on_input_type_rank_2(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        t2: &syn::Type,
        registry: &Registry<Self::Metadata>,
    ) -> Option<ConverterImpl<Self::Metadata>>;

    fn on_input_type_rank_3(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        t2: &syn::Type,
        t3: &syn::Type,
        registry: &Registry<Self::Metadata>,
    ) -> Option<ConverterImpl<Self::Metadata>>;

    /// Source types accepted at `impl Into<target> + Send + 'static`
    /// parameters. The caller is fully responsible for the list — if
    /// the identity arm `target → target` is wanted, spell it out
    /// with [`IntoSource::borrow`] / [`IntoSource::consume`]. The
    /// resolver does **not** auto-prepend an identity arm.
    ///
    /// Default: no sources. Wrappers override (match on `target`) to
    /// declare project-specific source types, e.g. `String → KeyExpr`
    /// via `TryFrom<String>`. The returned vector's order determines
    /// the runtime dispatch order in the emitted converter.
    fn into_sources(&self, target: &syn::Type) -> Vec<IntoSource> {
        let _ = target;
        Vec::new()
    }

    /// Build the dispatcher converter for an
    /// `impl Into<target> + Send + 'static` parameter, given the
    /// source list returned by [`Self::into_sources`]. The resolver
    /// calls this only after [`Self::on_input_type_rank_1`] has
    /// returned `None` for the Into pattern, so wrappers that need
    /// full custom dispatch can intercept earlier and skip this path.
    ///
    /// Default: `None`. Backends that support Into-source dispatch
    /// (e.g. [`crate::jni::JniExt`]) override this to delegate to
    /// their own emitter such as
    /// [`crate::jni::JniExt::emit_into_dispatcher`].
    fn dispatch_into_input(
        &self,
        target: &syn::Type,
        sources: &[IntoSource],
        registry: &Registry<Self::Metadata>,
    ) -> Option<ConverterImpl<Self::Metadata>> {
        let _ = (target, sources, registry);
        None
    }

    /// Build the wrapper converter for an
    /// `impl Fn(args...) + Send + Sync + 'static` parameter, given the
    /// already-extracted arg types in declaration order. The resolver
    /// calls this only after [`Self::on_input_type_rank_0`] /
    /// [`Self::on_input_type_rank_1`] / [`Self::on_input_type_rank_2`] /
    /// [`Self::on_input_type_rank_3`] (for the appropriate arity) has
    /// returned `None`, so wrappers that need custom callback dispatch
    /// can intercept earlier and skip this path.
    ///
    /// `args` are the rust-side argument types as they appear in the
    /// source signature. Note that callback args flow inverse to the
    /// callback parameter itself: the callback parameter is *input*,
    /// but its args are produced by the rust side and consumed by the
    /// foreign side, so they are *output* direction for converter
    /// resolution. The framework handles this direction-flip at
    /// registration time (`register_type_inner` in `core::registry`),
    /// so implementations of this method should look up
    /// already-registered *output* converters for each arg type.
    ///
    /// Default: `None`. Backends that support `impl Fn` callbacks
    /// (e.g. [`crate::jni::JniExt`]) override this.
    fn dispatch_fn_input(
        &self,
        args: &[syn::Type],
        registry: &Registry<Self::Metadata>,
    ) -> Option<ConverterImpl<Self::Metadata>> {
        let _ = (args, registry);
        None
    }

    // ── Output direction (rust → wire) ─────────────────────────────

    /// Whole-type output converter.
    fn on_output_type_rank_0(
        &self,
        ty: &syn::Type,
        registry: &Registry<Self::Metadata>,
    ) -> Option<ConverterImpl<Self::Metadata>>;

    fn on_output_type_rank_1(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        registry: &Registry<Self::Metadata>,
    ) -> Option<ConverterImpl<Self::Metadata>>;

    fn on_output_type_rank_2(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        t2: &syn::Type,
        registry: &Registry<Self::Metadata>,
    ) -> Option<ConverterImpl<Self::Metadata>>;

    fn on_output_type_rank_3(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        t2: &syn::Type,
        t3: &syn::Type,
        registry: &Registry<Self::Metadata>,
    ) -> Option<ConverterImpl<Self::Metadata>>;
}
