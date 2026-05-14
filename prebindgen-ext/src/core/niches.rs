//! Niche optimisation for FFI-wire encodings.
//!
//! A *niche* is a bit-pattern that the wire type *can* represent but that
//! a particular converter is guaranteed to never produce on output and
//! always reject on input. Wrappers like `Option<_>` and sum-typed enums
//! carve niches one at a time for their own discriminants and re-export
//! the remainder so further wrappers stack.
//!
//! Direct analogy with Rust's niche optimisation:
//!
//! | Rust                         | This crate                              |
//! | ---------------------------- | --------------------------------------- |
//! | `NonZeroU32` declares `{0}`  | converter sets `niches = Niches::one(…)`|
//! | `Option<NonZeroU32>` is u32  | `Option<T>` reuses inner's wire         |
//! | `Option<Option<NonZeroU32>>` | falls back unless inner exposes ≥2      |
//!
//! In the FFI setting the canonical example is a Rust value encoded as a
//! raw `Arc::into_raw` pointer carried over the wire as `jlong`: real
//! `Arc::into_raw` results are never `0`, so the converter declares the
//! single niche `{0}`. `Option<T>` then automatically reuses the same
//! `jlong` wire with `0` meaning `None`, matching the C-pointer-with-null
//! ABI most JNI bindings already use.
//!
//! ## Cascading
//!
//! [`Niches::carve`] returns the next slot together with the remainder.
//! The wrapper places the carved value into its own emitted code (output:
//! `None` is encoded as `slot.value`; input: `slot.matches` is the
//! discriminator predicate) and stores `rest` on its own
//! [`crate::core::prebindgen_ext::ConverterImpl::niches`] so any
//! enclosing wrapper can keep carving. Once `rest` is empty further
//! wrappers must fall back to a tag/box scheme.
//!
//! ## Soundness
//!
//! For the carve to be sound, the inner converter's outputs must
//! genuinely avoid the carved bit pattern, and its input must reject it
//! (typically by erroring). The plugin author guarantees this — `Niches`
//! is a *declaration* that the resolver and wrappers trust.
//!
//! ## Calling convention for `matches`
//!
//! The `matches` predicate is spliced into the input wrapper's body where
//! the wire-typed parameter `v` is in scope. The exact shape of `v`
//! depends on the wire kind:
//!
//! * Standard wires (e.g. `JObject`, `jlong`): `v: &<wire>` — write
//!   `*v == 0` for `jlong`, `v.is_null()` for `JObject` (autoderef).
//! * Raw-pointer wires (`*const T`): `v: <wire>` — write `v.is_null()`
//!   directly, no `*` deref.
//!
//! The plugin producing the niche knows which wire kind it is using and
//! must write `matches` accordingly.
//!
//! `value` is a wire-typed *constant* expression with no `v`, no `env` —
//! just the bit pattern (e.g. `0i64`, `jni::objects::JObject::null()`,
//! `std::ptr::null()`).

/// One free bit-pattern slot in the wire encoding.
///
/// See the module-level docs for the calling convention of `matches` and
/// `value`.
#[derive(Clone)]
pub struct NicheSlot {
    /// Wire-typed constant expression evaluating to this niche's bit
    /// pattern. Used by output wrappers to emit the discriminant.
    pub value: syn::Expr,
    /// Predicate testing whether the wire value `v` (in the local
    /// wrapper convention — see module docs) is *this* slot. Used by
    /// input wrappers to detect the discriminant.
    pub matches: syn::Expr,
}

/// An ordered set of [`NicheSlot`]s that a converter's wire type can
/// represent but that this converter never produces (output) and always
/// rejects (input).
///
/// Ordering: the *first* slot is the next one taken by [`Self::carve`].
/// Wrappers carve from the front; the remaining slots are passed up so
/// that further wrappers can stack their own discriminants.
#[derive(Clone, Default)]
pub struct Niches {
    pub slots: Vec<NicheSlot>,
}

impl Niches {
    /// No free bit-patterns. The default for converters whose wire
    /// encoding uses every bit-pattern as a valid value (e.g. `i64`
    /// over `jlong`).
    pub fn empty() -> Self {
        Self::default()
    }

    /// Convenience for the common single-niche case.
    pub fn one(value: syn::Expr, matches: syn::Expr) -> Self {
        Self { slots: vec![NicheSlot { value, matches }] }
    }

    /// Build from any iterable of slots; ordering is preserved.
    pub fn from_slots<I: IntoIterator<Item = NicheSlot>>(slots: I) -> Self {
        Self { slots: slots.into_iter().collect() }
    }

    /// Take the first slot for use as a wrapper's discriminant. Returns
    /// the carved slot and the remaining niches (which the wrapper
    /// should re-export on its own [`ConverterImpl`]). `None` if the
    /// set is empty — the caller must fall back to a tag/box scheme.
    pub fn carve(mut self) -> Option<(NicheSlot, Niches)> {
        if self.slots.is_empty() {
            None
        } else {
            let head = self.slots.remove(0);
            Some((head, self))
        }
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::ToTokens;

    fn slot_strs(s: &NicheSlot) -> (String, String) {
        (
            s.value.to_token_stream().to_string(),
            s.matches.to_token_stream().to_string(),
        )
    }

    #[test]
    fn empty_is_empty() {
        let n = Niches::empty();
        assert!(n.is_empty());
        assert_eq!(n.len(), 0);
        assert!(n.carve().is_none());
    }

    #[test]
    fn one_constructs_single_slot() {
        let n = Niches::one(syn::parse_quote!(0i64), syn::parse_quote!(*v == 0));
        assert_eq!(n.len(), 1);
        let (slot, rest) = n.carve().unwrap();
        let (val, pred) = slot_strs(&slot);
        assert_eq!(val, "0i64");
        assert_eq!(pred, "* v == 0");
        assert!(rest.is_empty());
    }

    #[test]
    fn from_slots_preserves_order() {
        let n = Niches::from_slots([
            NicheSlot { value: syn::parse_quote!(0i32),  matches: syn::parse_quote!(*v == 0) },
            NicheSlot { value: syn::parse_quote!(-1i32), matches: syn::parse_quote!(*v == -1) },
            NicheSlot { value: syn::parse_quote!(99i32), matches: syn::parse_quote!(*v == 99) },
        ]);
        assert_eq!(n.len(), 3);
        let (s0, n) = n.carve().unwrap();
        assert_eq!(slot_strs(&s0).0, "0i32");
        let (s1, n) = n.carve().unwrap();
        assert_eq!(slot_strs(&s1).0, "- 1i32");
        let (s2, n) = n.carve().unwrap();
        assert_eq!(slot_strs(&s2).0, "99i32");
        assert!(n.is_empty());
    }

    /// Carving propagates the remainder, allowing wrappers to stack.
    /// This mirrors `Option<Option<TypeWithTwoNiches>>` collapsing to
    /// the same wire as the inner type.
    #[test]
    fn cascading_carve() {
        let n = Niches::from_slots([
            NicheSlot { value: syn::parse_quote!(jni::sys::jint::MIN), matches: syn::parse_quote!(*v == jni::sys::jint::MIN) },
            NicheSlot { value: syn::parse_quote!(jni::sys::jint::MAX), matches: syn::parse_quote!(*v == jni::sys::jint::MAX) },
        ]);

        // Outer wrapper takes the first niche.
        let (outer, rest1) = n.carve().unwrap();
        assert_eq!(slot_strs(&outer).0, "jni :: sys :: jint :: MIN");
        assert_eq!(rest1.len(), 1);

        // Inner wrapper (carving from `rest1`) takes the second.
        let (inner, rest2) = rest1.carve().unwrap();
        assert_eq!(slot_strs(&inner).0, "jni :: sys :: jint :: MAX");
        assert!(rest2.is_empty());
    }

    /// `Niches::default()` equivalence to `empty()`.
    #[test]
    fn default_is_empty() {
        let n = Niches::default();
        assert!(n.is_empty());
    }

    /// Cloning produces independent ownership; carving the clone
    /// doesn't disturb the original (each carve consumes by value).
    #[test]
    fn clone_independence() {
        let original = Niches::one(syn::parse_quote!(0i32), syn::parse_quote!(*v == 0));
        let cloned = original.clone();
        let (_slot, rest) = cloned.carve().unwrap();
        assert!(rest.is_empty());
        assert_eq!(original.len(), 1, "original unaffected by clone's carve");
    }
}
