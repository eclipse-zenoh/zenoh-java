//! Single owner of everything parsed from the prebindgen source stream.
//!
//! [`Registry`] holds:
//! * Item maps (`functions`, `structs`, `enums`, `consts`) indexed by ident.
//!   Duplicate names across kinds OR within a kind are an error — prebindgen
//!   items live in one flat namespace.
//! * `passthrough` — items that aren't function/struct/enum/const (use, mod,
//!   type alias, macro_rules) emitted verbatim.
//! * `input_types` / `output_types` — type tables split by rank
//!   (`[HashMap<TypeKey, Option<TypeEntry>>; 4]`). Each type encountered in
//!   a `#[prebindgen]` fn signature or struct/enum body lands here.
//!
//! See the plan at `~/.claude/plans/are-there-any-reasons-hazy-brook.md` for
//! the full rationale.

use std::collections::{HashMap, HashSet};
use std::fmt;

use prebindgen::SourceLocation;
use quote::ToTokens;

use crate::core::niches::Niches;
use crate::core::prebindgen_ext::{IntoSource, Stage};

/// Canonical type-shape key — the `to_token_stream().to_string()` form of a
/// `syn::Type`. Whitespace-normalised (`"Vec<u8>"` and `"Vec < u8 >"` produce
/// the same key after parse-and-restringify).
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TypeKey(String);

impl TypeKey {
    /// Build a key by parsing the input as a type and re-serialising. Panics
    /// if the input does not parse as a `syn::Type`.
    pub fn parse(s: &str) -> Self {
        let ty: syn::Type = syn::parse_str(s)
            .unwrap_or_else(|e| panic!("TypeKey::parse: invalid type `{}`: {}", s, e));
        Self::from_type(&ty)
    }

    /// Build a key directly from a `syn::Type`.
    pub fn from_type(ty: &syn::Type) -> Self {
        Self(ty.to_token_stream().to_string())
    }

    /// The canonical string form.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Parse the key back into a `syn::Type`. Always succeeds because the
    /// key was originally constructed from a parseable type.
    pub fn to_type(&self) -> syn::Type {
        syn::parse_str(&self.0).unwrap_or_else(|e| {
            panic!("TypeKey::to_type: stored key `{}` no longer parses: {}", self.0, e)
        })
    }
}

impl fmt::Display for TypeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Per-cell registry entry.
#[derive(Clone)]
pub struct TypeEntry<M = ()> {
    /// Wire/destination type (e.g. `jni::sys::jlong`). Other converters
    /// that ask "what's the wire form of this rust type?" read this.
    pub destination: syn::Type,
    /// Complete generated function for the **wire-facing** stage of the
    /// converter (signature, body, attributes, lifetimes). Plugin owns
    /// the shape. Callers compute this stage's name via
    /// `function.sig.ident`.
    pub function: syn::ItemFn,
    /// **Rust-side** stages that compose with [`Self::function`] to form
    /// the full chain — copied verbatim from the resolving
    /// [`crate::core::prebindgen_ext::ConverterImpl::pre_stages`]. See
    /// that field's docs for the chain-order semantics.
    pub pre_stages: Vec<Stage>,
    /// Inner types whose function delegates to their converters. Empty for
    /// rank-0 resolutions; equal to the rank-N `subs` array for rank-N≥1
    /// resolutions. Used by the post-resolution propagation pass.
    pub subs: Vec<TypeKey>,
    /// Initially true for types that appear directly in a `#[prebindgen]` fn
    /// signature; false for sub-positions. Promoted true by the propagation
    /// pass for any type reachable via `subs` from another required type.
    pub required: bool,
    /// Wire bit-patterns this converter never produces / always rejects.
    /// Wrappers (`Option<_>`, sum-typed enums) carve from this set for
    /// their own discriminants. See [`Niches`] for the cascade model.
    pub niches: Niches,
    /// Source-arm metadata for `impl Into<target>` dispatcher entries —
    /// `Some(...)` only when this entry was produced by
    /// [`crate::core::prebindgen_ext::PrebindgenExt::dispatch_into_input`].
    /// Language-side wrapper emitters (e.g. the Kotlin emitter in
    /// `jni_kotlin_ext.rs`) read this to drive per-source `is JNI<X>`
    /// fan-out — one Java-side arm per declared
    /// [`crate::core::prebindgen_ext::IntoSource`] with the source's
    /// borrow/consume mode. `None` for every non-dispatcher entry.
    pub into_sources: Option<Vec<IntoSource>>,
    /// Language-specific extras carried in by the
    /// [`crate::core::prebindgen_ext::ConverterImpl`] that filled this
    /// slot. Emitter code reads this directly — the registry is the
    /// single source of truth for cross-language facts (Kotlin names,
    /// C header names, etc.). Defaults to `()` for back-ends that don't
    /// need any.
    pub metadata: M,
}

/// Direction of a converter pair.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum Direction {
    /// Wire → Rust.
    Input,
    /// Rust → Wire.
    Output,
}

impl Direction {
    pub fn flip(self) -> Self {
        match self {
            Direction::Input => Direction::Output,
            Direction::Output => Direction::Input,
        }
    }
}

/// Maximum rank the resolver supports (rank 0..=3).
pub const MAX_RANK: usize = 3;

/// Single owner of everything parsed from the prebindgen source stream.
///
/// The metadata parameter `M` is the language back-end's per-converter
/// extra type, supplied via
/// [`crate::core::prebindgen_ext::PrebindgenExt::Metadata`]. Each
/// [`TypeEntry`] carries one `M` copied in by the resolver from the
/// [`crate::core::prebindgen_ext::ConverterImpl`] that produced it.
/// Back-ends that don't carry extras leave `M = ()`.
pub struct Registry<M = ()> {
    pub functions: HashMap<syn::Ident, (syn::ItemFn, SourceLocation)>,
    pub structs: HashMap<syn::Ident, (syn::ItemStruct, SourceLocation)>,
    pub enums: HashMap<syn::Ident, (syn::ItemEnum, SourceLocation)>,
    pub consts: HashMap<syn::Ident, (syn::ItemConst, SourceLocation)>,
    /// Anything else (use, mod, type alias, macro_rules) — passed through.
    pub passthrough: Vec<(syn::Item, SourceLocation)>,

    /// Type tables. `input_types[N]` holds types whose rank is exactly `N`.
    /// A given key appears in exactly one bucket.
    pub input_types: [HashMap<TypeKey, Option<TypeEntry<M>>>; 4],
    pub output_types: [HashMap<TypeKey, Option<TypeEntry<M>>>; 4],

    /// First-seen source location for each type key. Used in error messages
    /// to point the user at where a required-but-unresolved type came from.
    pub type_locations: HashMap<TypeKey, SourceLocation>,

    /// Sidecar tracking which keys were registered as top-level fn-signature
    /// types, separate from per-entry `required` (which the resolver flips
    /// into `TypeEntry::required` once an entry is filled).
    pub required_inputs_scan: HashSet<TypeKey>,
    pub required_outputs_scan: HashSet<TypeKey>,
}

impl<M> Default for Registry<M> {
    fn default() -> Self {
        Self {
            functions: HashMap::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
            consts: HashMap::new(),
            passthrough: Vec::new(),
            input_types: Default::default(),
            output_types: Default::default(),
            type_locations: HashMap::new(),
            required_inputs_scan: HashSet::new(),
            required_outputs_scan: HashSet::new(),
        }
    }
}


/// Errors surfaced by the scan phase.
#[derive(Debug)]
pub enum ScanError {
    DuplicateName {
        name: syn::Ident,
        first: SourceLocation,
        second: SourceLocation,
    },
    DisallowedImplTrait {
        ty: String,
        loc: SourceLocation,
    },
    UnsupportedReceiver {
        loc: SourceLocation,
    },
    UnsupportedParamPattern {
        loc: SourceLocation,
    },
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScanError::DuplicateName { name, first, second } => write!(
                f,
                "duplicate prebindgen name `{}`: first at {}, second at {}",
                name, first, second
            ),
            ScanError::DisallowedImplTrait { ty, loc } => write!(
                f,
                "`impl Trait` is not allowed at {}: `{}` (only `impl Fn(...) + Send + Sync + 'static` is supported)",
                loc, ty
            ),
            ScanError::UnsupportedReceiver { loc } => {
                write!(f, "method receiver (`self`) parameters are not supported at {}", loc)
            }
            ScanError::UnsupportedParamPattern { loc } => {
                write!(f, "non-ident parameter pattern is not supported at {}", loc)
            }
        }
    }
}

impl std::error::Error for ScanError {}

/// Combined error surfaced by [`Registry::write_rust`].
#[derive(Debug)]
pub enum WriteRustError {
    Scan(ScanError),
    Resolve(crate::core::resolve::ResolveError),
    Write(crate::core::write::WriteError),
}

impl fmt::Display for WriteRustError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WriteRustError::Scan(e) => write!(f, "{}", e),
            WriteRustError::Resolve(e) => write!(f, "{}", e),
            WriteRustError::Write(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for WriteRustError {}

impl From<ScanError> for WriteRustError {
    fn from(e: ScanError) -> Self {
        WriteRustError::Scan(e)
    }
}

impl From<crate::core::resolve::ResolveError> for WriteRustError {
    fn from(e: crate::core::resolve::ResolveError) -> Self {
        WriteRustError::Resolve(e)
    }
}

impl From<crate::core::write::WriteError> for WriteRustError {
    fn from(e: crate::core::write::WriteError) -> Self {
        WriteRustError::Write(e)
    }
}

impl<M> Registry<M> {
    /// Construct a `Registry` by indexing a stream of source items.
    ///
    /// Callers feed any `(syn::Item, SourceLocation)` iterator — typically
    /// `source.items_all()`, `source.items_except_groups(...)`, or a
    /// hand-rolled filter chain — so item-level selection happens upstream
    /// of the registry rather than inside it.
    ///
    /// This step only populates the item maps (`functions`, `structs`,
    /// `enums`, `consts`, `passthrough`). Signature/body scanning that
    /// drives type-resolution requirements happens later, in
    /// [`Self::scan_declared`], and is gated on what the language ext
    /// has explicitly declared. Items that are never declared remain in
    /// the registry but never drive type resolution and never emit.
    pub fn from_items<I>(items: I) -> Result<Self, ScanError>
    where
        I: IntoIterator<Item = (syn::Item, SourceLocation)>,
    {
        let mut registry = Registry::default();
        for (item, loc) in items {
            registry.index_item(item, loc)?;
        }
        Ok(registry)
    }

    /// Scan the signature/body of every item declared by `ext`.
    ///
    /// * For each ident in `ext.declared_functions()` ∩ indexed functions,
    ///   call [`Self::scan_fn_signature`] so parameter and return types
    ///   are registered as required.
    /// * For each `TypeKey` in `ext.declared_types()`, mark the key as
    ///   required in both directions; if the key resolves to an indexed
    ///   struct/enum, also scan its body so field types are registered
    ///   (still `required: false` — propagation later promotes them
    ///   through `subs`).
    ///
    /// Declared items that don't match any indexed body get a build
    /// warning (likely a typo in the build script). Indexed items that
    /// were never declared also get a `cargo:warning=` skip line so the
    /// user sees the full skip list per build.
    pub fn scan_declared<E>(&mut self, ext: &E) -> Result<(), ScanError>
    where
        E: crate::core::prebindgen_ext::PrebindgenExt<Metadata = M>,
    {
        let declared_fns = ext.declared_functions();
        let declared_types = ext.declared_types();

        // Scan declared functions.
        for ident in &declared_fns {
            if let Some((item_fn, loc)) = self.functions.get(ident).cloned() {
                self.scan_fn_signature(&item_fn, &loc)?;
            } else {
                println!(
                    "cargo:warning=prebindgen-ext: declared function `{}` not found among #[prebindgen] items",
                    ident
                );
            }
        }

        // Scan declared types.
        for key in &declared_types {
            let ty = key.to_type();
            let mut matched = false;
            if let Some(ident) = type_path_tail_ident(&ty) {
                if let Some((s, loc)) = self.structs.get(&ident).cloned() {
                    self.scan_struct(&s, &loc)?;
                    self.ensure_entry(Direction::Input, &ty, true, &loc);
                    self.ensure_entry(Direction::Output, &ty, true, &loc);
                    matched = true;
                } else if let Some((e, loc)) = self.enums.get(&ident).cloned() {
                    self.scan_enum(&e, &loc)?;
                    self.ensure_entry(Direction::Input, &ty, true, &loc);
                    self.ensure_entry(Direction::Output, &ty, true, &loc);
                    matched = true;
                }
            }
            if !matched {
                // Declared type without an indexed body (e.g.
                // `ptr_class(ZKeyExpr<'static>)` on a re-exported
                // foreign type). Still mark required so the resolver
                // tries to produce a converter for it.
                let loc = self.type_locations.get(key).cloned().unwrap_or_default();
                self.ensure_entry(Direction::Input, &ty, true, &loc);
                self.ensure_entry(Direction::Output, &ty, true, &loc);
            }
        }

        // Warn about indexed items that the ext never claimed.
        let mut skipped_fns: Vec<String> = self
            .functions
            .keys()
            .filter(|k| !declared_fns.contains(*k))
            .map(|k| k.to_string())
            .collect();
        skipped_fns.sort();
        for name in &skipped_fns {
            println!(
                "cargo:warning=prebindgen-ext: skipping undeclared #[prebindgen] fn `{}`",
                name
            );
        }

        let mut skipped_types: Vec<String> = Vec::new();
        for ident in self.structs.keys() {
            let key = TypeKey::parse(&ident.to_string());
            if !declared_types.contains(&key) {
                skipped_types.push(ident.to_string());
            }
        }
        for ident in self.enums.keys() {
            let key = TypeKey::parse(&ident.to_string());
            if !declared_types.contains(&key) {
                skipped_types.push(ident.to_string());
            }
        }
        skipped_types.sort();
        for name in &skipped_types {
            println!(
                "cargo:warning=prebindgen-ext: skipping undeclared #[prebindgen] struct/enum `{}`",
                name
            );
        }

        Ok(())
    }

    /// True iff the key was scanned as a top-level fn-signature input type.
    pub fn is_required_input_at_scan(&self, key: &TypeKey) -> bool {
        self.required_inputs_scan.contains(key)
    }
    pub fn is_required_output_at_scan(&self, key: &TypeKey) -> bool {
        self.required_outputs_scan.contains(key)
    }

    /// Look up the resolved input entry for `ty`, returning `None` if it
    /// was never registered or is still unresolved. The returned entry's
    /// `function.sig.ident` is the converter's call name; `destination` is
    /// its wire form.
    pub fn input_entry(&self, ty: &syn::Type) -> Option<&TypeEntry<M>> {
        let key = TypeKey::from_type(ty);
        for bucket in &self.input_types {
            if let Some(slot) = bucket.get(&key) {
                return slot.as_ref();
            }
        }
        None
    }

    /// Look up the resolved output entry for `ty`. See [`Self::input_entry`].
    pub fn output_entry(&self, ty: &syn::Type) -> Option<&TypeEntry<M>> {
        let key = TypeKey::from_type(ty);
        for bucket in &self.output_types {
            if let Some(slot) = bucket.get(&key) {
                return slot.as_ref();
            }
        }
        None
    }

    fn index_item(&mut self, item: syn::Item, loc: SourceLocation) -> Result<(), ScanError> {
        match item {
            syn::Item::Fn(f) => {
                self.check_no_duplicate(&f.sig.ident, &loc)?;
                self.functions.insert(f.sig.ident.clone(), (f, loc));
                Ok(())
            }
            syn::Item::Struct(s) => {
                self.check_no_duplicate(&s.ident, &loc)?;
                self.structs.insert(s.ident.clone(), (s, loc));
                Ok(())
            }
            syn::Item::Enum(e) => {
                self.check_no_duplicate(&e.ident, &loc)?;
                self.enums.insert(e.ident.clone(), (e, loc));
                Ok(())
            }
            syn::Item::Const(c) => {
                self.check_no_duplicate(&c.ident, &loc)?;
                self.consts.insert(c.ident.clone(), (c, loc));
                Ok(())
            }
            other => {
                self.passthrough.push((other, loc));
                Ok(())
            }
        }
    }

    fn check_no_duplicate(&self, name: &syn::Ident, loc: &SourceLocation) -> Result<(), ScanError> {
        if let Some(first) = self.first_seen_loc(name) {
            return Err(ScanError::DuplicateName {
                name: name.clone(),
                first,
                second: loc.clone(),
            });
        }
        Ok(())
    }

    fn first_seen_loc(&self, name: &syn::Ident) -> Option<SourceLocation> {
        if let Some((_, loc)) = self.functions.get(name) { return Some(loc.clone()); }
        if let Some((_, loc)) = self.structs.get(name)   { return Some(loc.clone()); }
        if let Some((_, loc)) = self.enums.get(name)     { return Some(loc.clone()); }
        if let Some((_, loc)) = self.consts.get(name)    { return Some(loc.clone()); }
        None
    }

    fn scan_fn_signature(&mut self, f: &syn::ItemFn, loc: &SourceLocation) -> Result<(), ScanError> {
        // Mechanical: register every fn-signature type as the user wrote it.
        // No semantic transformations (no &T→T strip, no ZResult<T>→T strip,
        // no skip for () / ZResult<()>). The plugin handles those via rank
        // handlers; propagation through `subs` then marks transitive deps
        // (e.g. &Foo's `& _` rank-1 handler returns subs=[Foo], so Foo
        // becomes required).
        for input in &f.sig.inputs {
            match input {
                syn::FnArg::Receiver(_) => {
                    return Err(ScanError::UnsupportedReceiver { loc: loc.clone() });
                }
                syn::FnArg::Typed(pt) => {
                    if !matches!(&*pt.pat, syn::Pat::Ident(_)) {
                        return Err(ScanError::UnsupportedParamPattern { loc: loc.clone() });
                    }
                    self.register_type_recursive(Direction::Input, &*pt.ty, true, loc)?;
                }
            }
        }
        let ret_ty: syn::Type = match &f.sig.output {
            syn::ReturnType::Default => syn::parse_quote!(()),
            syn::ReturnType::Type(_, ty) => (**ty).clone(),
        };
        self.register_type_recursive(Direction::Output, &ret_ty, true, loc)?;
        Ok(())
    }

    fn scan_struct(&mut self, s: &syn::ItemStruct, loc: &SourceLocation) -> Result<(), ScanError> {
        // The struct itself can appear in either direction.
        let ty: syn::Type = syn::parse_str(&s.ident.to_string()).expect("ident is a valid type");
        self.ensure_entry(Direction::Input, &ty, false, loc);
        self.ensure_entry(Direction::Output, &ty, false, loc);

        if let syn::Fields::Named(named) = &s.fields {
            for field in &named.named {
                self.register_type_recursive(Direction::Input, &field.ty, false, loc)?;
                self.register_type_recursive(Direction::Output, &field.ty, false, loc)?;
            }
        }
        Ok(())
    }

    fn scan_enum(&mut self, e: &syn::ItemEnum, loc: &SourceLocation) -> Result<(), ScanError> {
        let ty: syn::Type = syn::parse_str(&e.ident.to_string()).expect("ident is a valid type");
        self.ensure_entry(Direction::Input, &ty, false, loc);
        self.ensure_entry(Direction::Output, &ty, false, loc);

        for variant in &e.variants {
            for field in &variant.fields {
                self.register_type_recursive(Direction::Input, &field.ty, false, loc)?;
                self.register_type_recursive(Direction::Output, &field.ty, false, loc)?;
            }
        }
        Ok(())
    }

    /// Register `ty` as an entry in the given direction, then recurse into
    /// every nested position. `top_required` applies only to `ty` itself;
    /// nested positions are always recorded as not-required.
    fn register_type_recursive(
        &mut self,
        dir: Direction,
        ty: &syn::Type,
        top_required: bool,
        loc: &SourceLocation,
    ) -> Result<(), ScanError> {
        let mut visited: HashSet<TypeKey> = HashSet::new();
        self.register_type_inner(dir, ty, top_required, loc, &mut visited)
    }

    fn register_type_inner(
        &mut self,
        dir: Direction,
        ty: &syn::Type,
        is_top: bool,
        loc: &SourceLocation,
        visited: &mut HashSet<TypeKey>,
    ) -> Result<(), ScanError> {
        // Reject `impl Trait` except `impl Fn(...) + Send + Sync + 'static`
        // and `impl Into<T> + Send + 'static`.
        if let syn::Type::ImplTrait(it) = ty {
            if extract_fn_trait_args(ty).is_none() && extract_into_trait_arg(ty).is_none() {
                return Err(ScanError::DisallowedImplTrait {
                    ty: it.to_token_stream().to_string(),
                    loc: loc.clone(),
                });
            }
        }

        let key = TypeKey::from_type(ty);
        if !visited.insert(key.clone()) {
            return Ok(()); // cycle guard
        }

        self.ensure_entry(dir, ty, is_top, loc);

        for (child_dir, sub) in self.immediate_edges(dir, ty) {
            self.register_type_inner(child_dir, &sub, false, loc, visited)?;
        }
        Ok(())
    }

    fn ensure_entry(
        &mut self,
        dir: Direction,
        ty: &syn::Type,
        required: bool,
        loc: &SourceLocation,
    ) {
        let key = TypeKey::from_type(ty);
        let rank = compute_rank(ty).min(MAX_RANK);
        let bucket = match dir {
            Direction::Input => &mut self.input_types[rank],
            Direction::Output => &mut self.output_types[rank],
        };
        bucket.entry(key.clone()).or_insert(None);
        if required {
            match dir {
                Direction::Input => self.required_inputs_scan.insert(key.clone()),
                Direction::Output => self.required_outputs_scan.insert(key.clone()),
            };
        }
        self.type_locations.entry(key).or_insert_with(|| loc.clone());
    }

    /// Enumerate the immediate type-graph edges out of `(dir, ty)`:
    /// generic args / Fn args / tuple elements / ref/array/slice/ptr targets,
    /// plus — if `ty` is the bare ident of an indexed struct or enum — the
    /// field types of that struct/enum.
    ///
    /// `impl Fn(args)` arg types flow with `dir.flip()`; everything else
    /// inherits `dir`. Used by both `register_type_inner` (during scan) and
    /// the unresolved-descendants BFS in `resolve` (for diagnostics).
    pub(crate) fn immediate_edges(
        &self,
        dir: Direction,
        ty: &syn::Type,
    ) -> Vec<(Direction, syn::Type)> {
        let mut out: Vec<(Direction, syn::Type)> = Vec::new();
        let (positions, child_dir) = if let Some(args) = extract_fn_trait_args(ty) {
            (args, dir.flip())
        } else {
            (immediate_subtype_positions(ty), dir)
        };
        for sub in positions {
            out.push((child_dir, sub));
        }
        if let Some(name) = type_path_tail_ident(ty) {
            if let Some((s, _)) = self.structs.get(&name) {
                if let syn::Fields::Named(named) = &s.fields {
                    for field in &named.named {
                        out.push((dir, field.ty.clone()));
                    }
                }
            }
            if let Some((e, _)) = self.enums.get(&name) {
                for variant in &e.variants {
                    for field in &variant.fields {
                        out.push((dir, field.ty.clone()));
                    }
                }
            }
        }
        out
    }

    /// One-shot: resolve every required type using `ext`, then write the
    /// generated Rust bindings file. The single public entry point for
    /// language-specific binding generation — language-agnostic because
    /// `ext` is any [`crate::core::prebindgen_ext::PrebindgenExt`] impl
    /// whose `Metadata` matches this registry's `M` parameter.
    pub fn write_rust<E>(
        &mut self,
        ext: &E,
        out_path: impl AsRef<std::path::Path>,
    ) -> Result<std::path::PathBuf, WriteRustError>
    where
        E: crate::core::prebindgen_ext::PrebindgenExt<Metadata = M>,
        M: Clone + Default,
    {
        self.scan_declared(ext)?;
        crate::core::resolve::resolve(self, ext)?;
        Ok(crate::core::write::write_rust(self, ext, out_path)?)
    }
}

// ──────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────

/// Number of leaves in a type's substitutable-position tree.
pub fn compute_rank(ty: &syn::Type) -> usize {
    let positions = immediate_subtype_positions(ty);
    if positions.is_empty() {
        return 0;
    }
    positions.iter().map(|p| std::cmp::max(1, compute_rank(p))).sum()
}

/// Immediate child type positions of `ty` (one level deep).
pub fn immediate_subtype_positions(ty: &syn::Type) -> Vec<syn::Type> {
    match ty {
        syn::Type::Path(p) => {
            if let Some(last) = p.path.segments.last() {
                if let syn::PathArguments::AngleBracketed(ab) = &last.arguments {
                    return ab
                        .args
                        .iter()
                        .filter_map(|a| {
                            if let syn::GenericArgument::Type(t) = a {
                                Some(t.clone())
                            } else {
                                None
                            }
                        })
                        .collect();
                }
            }
            vec![]
        }
        syn::Type::Reference(r) => vec![(*r.elem).clone()],
        syn::Type::Tuple(t) => t.elems.iter().cloned().collect(),
        syn::Type::Array(a) => vec![(*a.elem).clone()],
        syn::Type::Slice(s) => vec![(*s.elem).clone()],
        syn::Type::Ptr(p) => vec![(*p.elem).clone()],
        syn::Type::Group(g) => immediate_subtype_positions(&g.elem),
        syn::Type::Paren(p) => immediate_subtype_positions(&p.elem),
        syn::Type::ImplTrait(_) => extract_fn_trait_args(ty)
            .or_else(|| extract_into_trait_arg(ty).map(|t| vec![t]))
            .unwrap_or_default(),
        _ => vec![],
    }
}


/// If `ty` is exactly `impl Into<T> + Send + 'static`, return `T`. Any
/// other bound combination (missing `Send`/`'static`, extra traits, no
/// `Into`) returns `None` — the framework rejects bare `impl Into<T>`
/// at scan time.
///
/// Mirrors [`extract_fn_trait_args`] in shape and intent: a single
/// well-formed exception to the otherwise-blanket "no `impl Trait`"
/// rule, picked up by every framework helper that handles parameter
/// type recognition (scan, rank, wildcard enumeration, rebuild).
pub fn extract_into_trait_arg(ty: &syn::Type) -> Option<syn::Type> {
    let syn::Type::ImplTrait(it) = ty else {
        return None;
    };
    let mut target: Option<syn::Type> = None;
    let mut has_send = false;
    let mut has_static = false;
    for bound in &it.bounds {
        match bound {
            syn::TypeParamBound::Trait(tb) => {
                let last = tb.path.segments.last()?;
                let name = last.ident.to_string();
                match name.as_str() {
                    "Into" => {
                        let syn::PathArguments::AngleBracketed(ab) = &last.arguments else {
                            return None;
                        };
                        let mut tys = ab.args.iter().filter_map(|a| match a {
                            syn::GenericArgument::Type(t) => Some(t.clone()),
                            _ => None,
                        });
                        let t = tys.next()?;
                        if tys.next().is_some() {
                            return None;
                        }
                        target = Some(t);
                    }
                    "Send" => has_send = true,
                    _ => return None,
                }
            }
            syn::TypeParamBound::Lifetime(lt) if lt.ident == "static" => has_static = true,
            _ => return None,
        }
    }
    if has_send && has_static {
        target
    } else {
        None
    }
}

/// If `ty` is `impl Fn(T1, T2, ...) + Send + Sync + 'static`, return the
/// `Fn` argument types in declaration order. Otherwise None.
pub fn extract_fn_trait_args(ty: &syn::Type) -> Option<Vec<syn::Type>> {
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

/// Return the bare last-path-segment ident of `ty` if `ty` is a path type
/// like `Sample` (not generic). None for `Option<Sample>`, `&T`, `(A, B)`.
pub(crate) fn type_path_tail_ident(ty: &syn::Type) -> Option<syn::Ident> {
    if let syn::Type::Path(tp) = ty {
        if let Some(last) = tp.path.segments.last() {
            if matches!(last.arguments, syn::PathArguments::None) {
                return Some(last.ident.clone());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::niches::Niches;
    use crate::core::prebindgen_ext::{ConverterImpl, PrebindgenExt};
    use proc_macro2::TokenStream;
    use std::collections::HashSet;

    /// Minimal `PrebindgenExt` for scan-pipeline tests. Carries the
    /// declared sets the test wants and stubs every emission/converter
    /// hook into something inert.
    #[derive(Default)]
    struct StubExt {
        functions: HashSet<syn::Ident>,
        types: HashSet<TypeKey>,
    }

    impl PrebindgenExt for StubExt {
        type Metadata = ();

        fn declared_functions(&self) -> HashSet<syn::Ident> {
            self.functions.clone()
        }
        fn declared_types(&self) -> HashSet<TypeKey> {
            self.types.clone()
        }

        fn on_function(&self, _f: &syn::ItemFn, _registry: &Registry<()>) -> TokenStream {
            TokenStream::new()
        }
        fn on_struct(&self, _s: &syn::ItemStruct, _registry: &Registry<()>) -> TokenStream {
            TokenStream::new()
        }
        fn on_enum(&self, _e: &syn::ItemEnum, _registry: &Registry<()>) -> TokenStream {
            TokenStream::new()
        }
        fn on_input_type_rank_0(
            &self,
            _ty: &syn::Type,
            _registry: &Registry<()>,
        ) -> Option<ConverterImpl<()>> {
            None
        }
        fn on_input_type_rank_1(
            &self,
            _pat: &syn::Type,
            _t1: &syn::Type,
            _registry: &Registry<()>,
        ) -> Option<ConverterImpl<()>> {
            None
        }
        fn on_input_type_rank_2(
            &self,
            _pat: &syn::Type,
            _t1: &syn::Type,
            _t2: &syn::Type,
            _registry: &Registry<()>,
        ) -> Option<ConverterImpl<()>> {
            None
        }
        fn on_input_type_rank_3(
            &self,
            _pat: &syn::Type,
            _t1: &syn::Type,
            _t2: &syn::Type,
            _t3: &syn::Type,
            _registry: &Registry<()>,
        ) -> Option<ConverterImpl<()>> {
            None
        }
        fn on_output_type_rank_0(
            &self,
            _ty: &syn::Type,
            _registry: &Registry<()>,
        ) -> Option<ConverterImpl<()>> {
            None
        }
        fn on_output_type_rank_1(
            &self,
            _pat: &syn::Type,
            _t1: &syn::Type,
            _registry: &Registry<()>,
        ) -> Option<ConverterImpl<()>> {
            None
        }
        fn on_output_type_rank_2(
            &self,
            _pat: &syn::Type,
            _t1: &syn::Type,
            _t2: &syn::Type,
            _registry: &Registry<()>,
        ) -> Option<ConverterImpl<()>> {
            None
        }
        fn on_output_type_rank_3(
            &self,
            _pat: &syn::Type,
            _t1: &syn::Type,
            _t2: &syn::Type,
            _t3: &syn::Type,
            _registry: &Registry<()>,
        ) -> Option<ConverterImpl<()>> {
            None
        }
    }

    // suppress unused warning on Niches — kept available for richer tests
    #[allow(dead_code)]
    fn _force_niches_use() -> Niches {
        Niches::empty()
    }

    fn fn_item(src: &str) -> (syn::Item, SourceLocation) {
        let item: syn::ItemFn = syn::parse_str(src).expect("test fn parse");
        (syn::Item::Fn(item), SourceLocation::default())
    }

    #[test]
    fn from_items_does_not_scan_signatures() {
        // A `#[prebindgen]`-marked fn whose return is a bare `impl Foo`
        // would have failed `from_items` under the old code path
        // (ScanError::DisallowedImplTrait). Now `from_items` is index-
        // only and accepts it without complaint.
        let items = vec![fn_item(
            "fn bogus(x: u64) -> impl std::fmt::Debug { 0u64 }",
        )];
        let reg: Registry<()> = Registry::from_items(items).expect("from_items must succeed");
        assert!(reg.required_inputs_scan.is_empty());
        assert!(reg.required_outputs_scan.is_empty());
        // The fn is indexed but no types are pre-required.
        assert!(reg.functions.contains_key(&syn::parse_str("bogus").unwrap()));
    }

    #[test]
    fn scan_declared_empty_ext_marks_nothing_required() {
        let items = vec![fn_item("fn good(x: u64) -> u64 { x }")];
        let mut reg: Registry<()> = Registry::from_items(items).unwrap();
        let ext = StubExt::default();
        reg.scan_declared(&ext).expect("empty ext = no scan");
        assert!(reg.required_inputs_scan.is_empty());
        assert!(reg.required_outputs_scan.is_empty());
    }

    #[test]
    fn scan_declared_marks_types_required_only_for_declared_fns() {
        let items = vec![
            fn_item("fn a(x: u64) -> u64 { x }"),
            fn_item("fn b(x: u32) -> u32 { x }"),
        ];
        let mut reg: Registry<()> = Registry::from_items(items).unwrap();
        let mut ext = StubExt::default();
        ext.functions.insert(syn::parse_str("a").unwrap());
        reg.scan_declared(&ext).unwrap();
        assert!(reg.required_inputs_scan.contains(&TypeKey::parse("u64")));
        assert!(reg.required_outputs_scan.contains(&TypeKey::parse("u64")));
        assert!(!reg.required_inputs_scan.contains(&TypeKey::parse("u32")));
        assert!(!reg.required_outputs_scan.contains(&TypeKey::parse("u32")));
    }

    #[test]
    fn scan_declared_fails_disallowed_impl_trait_only_when_fn_declared() {
        let items = vec![fn_item(
            "fn bogus(x: u64) -> impl std::fmt::Debug { 0u64 }",
        )];
        let mut reg: Registry<()> = Registry::from_items(items).unwrap();

        // Empty ext: the bogus fn is not scanned, so no error.
        let empty = StubExt::default();
        assert!(reg.scan_declared(&empty).is_ok());

        // Declare the fn: scan now fires the disallowed-impl-Trait error.
        let mut ext = StubExt::default();
        ext.functions.insert(syn::parse_str("bogus").unwrap());
        match reg.scan_declared(&ext) {
            Err(ScanError::DisallowedImplTrait { .. }) => (),
            other => panic!("expected DisallowedImplTrait, got {:?}", other),
        }
    }
}
