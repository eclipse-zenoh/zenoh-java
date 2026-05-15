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

use prebindgen::{Source, SourceLocation};
use quote::ToTokens;

use crate::core::niches::Niches;

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
pub struct TypeEntry {
    /// Wire/destination type (e.g. `jni::sys::jlong`). Other converters
    /// that ask "what's the wire form of this rust type?" read this.
    pub destination: syn::Type,
    /// Complete generated function for the converter (signature, body,
    /// attributes, lifetimes). Plugin owns the shape. Callers compute the
    /// converter's name via `function.sig.ident`.
    pub function: syn::ItemFn,
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
}

/// Direction of a converter pair.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
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
pub struct Registry {
    pub functions: HashMap<syn::Ident, (syn::ItemFn, SourceLocation)>,
    pub structs: HashMap<syn::Ident, (syn::ItemStruct, SourceLocation)>,
    pub enums: HashMap<syn::Ident, (syn::ItemEnum, SourceLocation)>,
    pub consts: HashMap<syn::Ident, (syn::ItemConst, SourceLocation)>,
    /// Anything else (use, mod, type alias, macro_rules) — passed through.
    pub passthrough: Vec<(syn::Item, SourceLocation)>,

    /// Type tables. `input_types[N]` holds types whose rank is exactly `N`.
    /// A given key appears in exactly one bucket.
    pub input_types: [HashMap<TypeKey, Option<TypeEntry>>; 4],
    pub output_types: [HashMap<TypeKey, Option<TypeEntry>>; 4],

    /// First-seen source location for each type key. Used in error messages
    /// to point the user at where a required-but-unresolved type came from.
    pub type_locations: HashMap<TypeKey, SourceLocation>,

    /// Sidecar tracking which keys were registered as top-level fn-signature
    /// types, separate from per-entry `required` (which the resolver flips
    /// into `TypeEntry::required` once an entry is filled).
    pub required_inputs_scan: HashSet<TypeKey>,
    pub required_outputs_scan: HashSet<TypeKey>,
}

impl Default for Registry {
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

impl Registry {
    /// Construct a `Registry` by scanning a `prebindgen::Source`.
    pub fn from_source(source: &Source) -> Result<Self, ScanError> {
        let mut registry = Registry::default();

        // Phase 1 — index all items.
        for (item, loc) in source.items_all() {
            registry.index_item(item, loc)?;
        }

        // Phase 2a — function signatures.
        let fn_keys: Vec<_> = registry.functions.keys().cloned().collect();
        for name in fn_keys {
            let (item_fn, loc) = registry.functions.get(&name).cloned().unwrap();
            registry.scan_fn_signature(&item_fn, &loc)?;
        }
        // Phase 2b — struct/enum bodies (their types need converters too).
        let struct_keys: Vec<_> = registry.structs.keys().cloned().collect();
        for name in struct_keys {
            let (item_struct, loc) = registry.structs.get(&name).cloned().unwrap();
            registry.scan_struct(&item_struct, &loc)?;
        }
        let enum_keys: Vec<_> = registry.enums.keys().cloned().collect();
        for name in enum_keys {
            let (item_enum, loc) = registry.enums.get(&name).cloned().unwrap();
            registry.scan_enum(&item_enum, &loc)?;
        }

        Ok(registry)
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
    pub fn input_entry(&self, ty: &syn::Type) -> Option<&TypeEntry> {
        let key = TypeKey::from_type(ty);
        for bucket in &self.input_types {
            if let Some(slot) = bucket.get(&key) {
                return slot.as_ref();
            }
        }
        None
    }

    /// Look up the resolved output entry for `ty`. See [`Self::input_entry`].
    pub fn output_entry(&self, ty: &syn::Type) -> Option<&TypeEntry> {
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

        // Recurse into nested positions. For `impl Fn(args)`, args travel
        // INVERSE to the parent direction (callback args flow inverse to
        // the callback itself).
        let (positions, child_dir) = if let Some(args) = extract_fn_trait_args(ty) {
            (args, dir.flip())
        } else {
            (immediate_subtype_positions(ty), dir)
        };

        for sub in positions {
            self.register_type_inner(child_dir, &sub, false, loc, visited)?;
        }

        // If this type is the bare ident of a struct/enum we already
        // indexed, recurse into its fields/variants in the SAME direction.
        if let Some(name) = type_path_tail_ident(ty) {
            if let Some((s, _)) = self.structs.get(&name).cloned() {
                if let syn::Fields::Named(named) = &s.fields {
                    for field in &named.named {
                        self.register_type_inner(dir, &field.ty, false, loc, visited)?;
                    }
                }
            }
            if let Some((e, _)) = self.enums.get(&name).cloned() {
                for variant in &e.variants {
                    for field in &variant.fields {
                        self.register_type_inner(dir, &field.ty, false, loc, visited)?;
                    }
                }
            }
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
}

// ──────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────

/// Number of leaves in a type's substitutable-position tree.
pub fn compute_rank(ty: &syn::Type) -> usize {
    if let Some(args) = extract_fn_trait_args(ty) {
        return args.iter().map(|t| std::cmp::max(1, compute_rank(t))).sum();
    }
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
fn type_path_tail_ident(ty: &syn::Type) -> Option<syn::Ident> {
    if let syn::Type::Path(tp) = ty {
        if let Some(last) = tp.path.segments.last() {
            if matches!(last.arguments, syn::PathArguments::None) {
                return Some(last.ident.clone());
            }
        }
    }
    None
}
