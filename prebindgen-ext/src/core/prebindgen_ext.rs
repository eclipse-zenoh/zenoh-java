//! `PrebindgenExt` — the single extension point for the new pipeline.
//!
//! One method per `#[prebindgen]` item kind (`on_function`, `on_struct`,
//! `on_enum`, `on_const`) returning the wrapper Rust tokens to emit, plus a
//! family of converter methods split by direction and rank:
//!
//! * Input  (wire → rust): `on_input_type_rank_0..3`
//! * Output (rust → wire): `on_output_type_rank_0..3`
//!
//! Each converter method returns `Some((destination, body))` if the ext
//! handles the type, or `None` to fall through to higher-rank wildcard
//! attempts (and ultimately to an "unresolved required type" error if the
//! resolver can't fill the cell).
//!
//! Bodies are `syn::Expr`s assuming an in-scope parameter `v` of the source
//! type (input direction) or the destination type (output direction). They
//! produce a value of the opposite type. The resolver wraps every emitted
//! body into a uniformly-shaped wrapper fn at file emission time.

use proc_macro2::TokenStream;

use crate::core::registry::Registry;

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

    /// Whole-type input converter. Returns `Some((wire, body))` where
    /// `body` is a `syn::Expr` assuming in-scope parameter `v: <wire>` and
    /// produces a value of type `ty`.
    fn on_input_type_rank_0(
        &self,
        ty: &syn::Type,
        registry: &Registry,
    ) -> Option<(syn::Type, syn::Expr)>;

    /// Single-wildcard input pattern. `pat` contains one `_`; `t1` is the
    /// type the wildcard slot held in the original. The body may reference
    /// the inner converter via `crate::core::converter_name::input_name`.
    fn on_input_type_rank_1(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        registry: &Registry,
    ) -> Option<(syn::Type, syn::Expr)>;

    fn on_input_type_rank_2(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        t2: &syn::Type,
        registry: &Registry,
    ) -> Option<(syn::Type, syn::Expr)>;

    fn on_input_type_rank_3(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        t2: &syn::Type,
        t3: &syn::Type,
        registry: &Registry,
    ) -> Option<(syn::Type, syn::Expr)>;

    // ── Output direction (rust → wire) ─────────────────────────────

    /// Whole-type output converter. Returns `Some((wire, body))` where
    /// `body` is a `syn::Expr` assuming in-scope parameter `v: &<ty>` and
    /// produces a value of type `wire`.
    fn on_output_type_rank_0(
        &self,
        ty: &syn::Type,
        registry: &Registry,
    ) -> Option<(syn::Type, syn::Expr)>;

    fn on_output_type_rank_1(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        registry: &Registry,
    ) -> Option<(syn::Type, syn::Expr)>;

    fn on_output_type_rank_2(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        t2: &syn::Type,
        registry: &Registry,
    ) -> Option<(syn::Type, syn::Expr)>;

    fn on_output_type_rank_3(
        &self,
        pat: &syn::Type,
        t1: &syn::Type,
        t2: &syn::Type,
        t3: &syn::Type,
        registry: &Registry,
    ) -> Option<(syn::Type, syn::Expr)>;

    // ── Wrapper assembly ───────────────────────────────────────────

    /// Build the full converter `fn` for an input entry. The ext is
    /// responsible for the signature shape (e.g. JNI adds `env: &mut JNIEnv`
    /// as a leading parameter and wraps the body in an error-conversion
    /// closure). `body` assumes in-scope parameter `v` of type `wire`;
    /// returns a value of type `rust`.
    fn wrap_input_converter(
        &self,
        name: &syn::Ident,
        rust: &syn::Type,
        wire: &syn::Type,
        body: &syn::Expr,
    ) -> syn::ItemFn;

    /// Build the full converter `fn` for an output entry. `body` assumes
    /// in-scope parameter `v: &<rust>`; returns a value of type `wire`.
    fn wrap_output_converter(
        &self,
        name: &syn::Ident,
        rust: &syn::Type,
        wire: &syn::Type,
        body: &syn::Expr,
    ) -> syn::ItemFn;
}
