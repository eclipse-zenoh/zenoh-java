//! Universal struct-processing converter.
//!
//! [`TypesConverter`] consumes `#[prebindgen]` `syn::ItemStruct`s from the
//! source iterator and delegates per-struct emission to a configured
//! [`StructStrategy`]. The strategy decides what items are emitted (a JNI
//! decoder fn, a flattened repr-C copy, a transmute assertion, …) and
//! optionally registers a [`TypeBinding`] in the shared registry.

use std::collections::VecDeque;

use quote::ToTokens;

use prebindgen::SourceLocation;

use crate::core::inline_fn::{InputFn, NO_INPUT, NO_OUTPUT, OutputFn};
use crate::core::type_registry::TypeRegistry;

/// Strategy for translating one `#[prebindgen]` struct into output items.
pub trait StructStrategy {
    /// Process a struct. The strategy:
    /// * may push zero or more output items into `out` (each paired with
    ///   the original `loc`);
    /// * may register or replace [`TypeBinding`]s in `registry`.
    fn process(
        &self,
        s: &syn::ItemStruct,
        loc: &SourceLocation,
        registry: &mut TypeRegistry,
        out: &mut Vec<(syn::Item, SourceLocation)>,
    );
}

/// Builder for [`TypesConverter`].
pub struct TypesBuilder {
    pub(crate) types: TypeRegistry,
    pub(crate) strategy: Box<dyn StructStrategy>,
}

impl TypesBuilder {
    fn new(strategy: Box<dyn StructStrategy>) -> Self {
        Self {
            types: TypeRegistry::new(),
            strategy,
        }
    }

    /// Add or replace a Rust/Wire type pair in the local registry.
    pub fn add_type_pair(
        mut self,
        rust_type: impl AsRef<str>,
        wire_type: impl AsRef<str>,
    ) -> Self {
        let rust_type = rust_type.as_ref().to_owned();
        self.types = self.types.type_pair(
            &rust_type,
            wire_type,
            NO_INPUT,
            NO_OUTPUT,
        );
        self
    }

    /// Add or replace an input conversion function in the local registry.
    pub fn add_input_conversion_function(
        mut self,
        rust_type: impl AsRef<str>,
        decode: InputFn,
    ) -> Self {
        self.types = self
            .types
            .add_input_conversion_function(rust_type, decode);
        self
    }

    /// Add or replace an output conversion function in the local registry.
    pub fn add_output_conversion_function(
        mut self,
        rust_type: impl AsRef<str>,
        encode: OutputFn,
    ) -> Self {
        self.types = self
            .types
            .add_output_conversion_function(rust_type, encode);
        self
    }

    /// Merge a reusable [`TypeRegistry`] into the registry.
    pub fn type_registry(mut self, registry: TypeRegistry) -> Self {
        self.types.extend_from(registry);
        self
    }

    pub fn build(self) -> TypesConverter {
        TypesConverter {
            cfg: self,
            pending: VecDeque::new(),
            buffered: false,
        }
    }
}

/// Universal struct converter. Drains the source iterator on first call,
/// then yields converted items one at a time from its internal queue.
pub struct TypesConverter {
    pub(crate) cfg: TypesBuilder,
    pending: VecDeque<(syn::Item, SourceLocation)>,
    buffered: bool,
}

impl TypesConverter {
    /// Construct a builder configured with the given struct strategy.
    pub fn builder<S: StructStrategy + 'static>(strategy: S) -> TypesBuilder {
        TypesBuilder::new(Box::new(strategy))
    }

    /// Drain `iter` on the first call, convert each struct item via the
    /// strategy, and yield queued results on subsequent calls.
    pub fn call<I>(&mut self, iter: &mut I) -> Option<(syn::Item, SourceLocation)>
    where
        I: Iterator<Item = (syn::Item, SourceLocation)>,
    {
        if !self.buffered {
            self.buffered = true;
            for (item, loc) in iter.by_ref() {
                self.convert(item, loc);
            }
        }
        self.pending.pop_front()
    }

    /// Closure suitable for `itertools::batching`.
    pub fn as_closure<'a, I>(
        &'a mut self,
    ) -> impl FnMut(&mut I) -> Option<(syn::Item, SourceLocation)> + 'a
    where
        I: Iterator<Item = (syn::Item, SourceLocation)>,
    {
        move |iter| self.call(iter)
    }

    /// Consume the converter and return the populated [`TypeRegistry`].
    pub fn into_type_registry(self) -> TypeRegistry {
        self.cfg.types
    }

    fn convert(&mut self, item: syn::Item, loc: SourceLocation) {
        match item {
            syn::Item::Struct(s) => {
                let mut out = Vec::new();
                self.cfg
                    .strategy
                    .process(&s, &loc, &mut self.cfg.types, &mut out);
                for entry in out {
                    self.pending.push_back(entry);
                }
            }
            other => panic!(
                "TypesConverter received a non-struct item at {loc}: {}",
                other.to_token_stream()
            ),
        }
    }
}
