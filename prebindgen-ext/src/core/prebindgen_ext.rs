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
pub struct ConverterImpl {
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
}

/// Implemented by destination-language back-ends (e.g. JNI). The resolver
/// drives this trait to fill `Registry::input_types` / `output_types`
/// entries; the file emitter calls `on_function` / `on_struct` / `on_enum` /
/// `on_const` to produce per-item wrapper code.
pub trait PrebindgenExt {
    // ── Item methods ───────────────────────────────────────────────

    /// Wrap a `#[prebindgen]` fn into the destination-language wrapper
    /// (e.g. JNI `extern "C"` fn).
    fn on_function(&self, f: &syn::ItemFn, registry: &Registry) -> TokenStream;

    /// Per-struct emission. Typically empty for languages that get
    /// everything they need from auto-generated converters.
    fn on_struct(&self, s: &syn::ItemStruct, registry: &Registry) -> TokenStream;

    /// Per-enum emission.
    fn on_enum(&self, e: &syn::ItemEnum, registry: &Registry) -> TokenStream;

    /// Per-const emission. Default: pass-through.
    fn on_const(&self, c: &syn::ItemConst, _registry: &Registry) -> TokenStream {
        use quote::ToTokens;
        c.to_token_stream()
    }

    // ── Input direction (wire → rust) ──────────────────────────────

    /// Whole-type input converter. Returns `Some(ConverterImpl)` if the
    /// ext handles `ty`.
    fn on_input_type_rank_0(
        &self,
        ty: &syn::Type,
        registry: &Registry,
    ) -> Option<ConverterImpl>;

    /// Single-wildcard input pattern. `pat` contains one `_`; `t1` is the
    /// type the wildcard slot held in the original.
    fn on_input_type_rank_1(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        registry: &Registry,
    ) -> Option<ConverterImpl>;

    fn on_input_type_rank_2(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        t2: &syn::Type,
        registry: &Registry,
    ) -> Option<ConverterImpl>;

    fn on_input_type_rank_3(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        t2: &syn::Type,
        t3: &syn::Type,
        registry: &Registry,
    ) -> Option<ConverterImpl>;

    // ── Output direction (rust → wire) ─────────────────────────────

    /// Whole-type output converter.
    fn on_output_type_rank_0(
        &self,
        ty: &syn::Type,
        registry: &Registry,
    ) -> Option<ConverterImpl>;

    fn on_output_type_rank_1(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        registry: &Registry,
    ) -> Option<ConverterImpl>;

    fn on_output_type_rank_2(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        t2: &syn::Type,
        registry: &Registry,
    ) -> Option<ConverterImpl>;

    fn on_output_type_rank_3(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        t2: &syn::Type,
        t3: &syn::Type,
        registry: &Registry,
    ) -> Option<ConverterImpl>;
}
