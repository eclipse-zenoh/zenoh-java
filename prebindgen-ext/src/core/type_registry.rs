//! Universal collection of type bindings, keyed by the canonical
//! `to_token_stream()` form of the Rust type-shape.

use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::quote;

use crate::core::inline_fn::InlineFn;
use crate::core::type_binding::{canon_type, TypeBinding};

#[derive(Default, Clone)]
pub struct TypeRegistry {
    pub(crate) types: HashMap<String, TypeBinding>,
}

pub struct TypePairBuilder {
    registry: TypeRegistry,
    rust_type: String,
}

impl TypePairBuilder {
    /// Add or replace the input conversion function for the current
    /// Rust type pair.
    pub fn input(mut self, decode: InlineFn) -> Self {
        self.registry
            .add_input_conversion_function_mut(&self.rust_type, decode);
        self
    }

    /// Add or replace the output conversion function for the current
    /// Rust type pair.
    pub fn output(mut self, encode: InlineFn) -> Self {
        self.registry
            .add_output_conversion_function_mut(&self.rust_type, encode);
        self
    }

    /// Add or replace a new Rust/Wire type pair and continue chaining
    /// conversions against that new pair.
    pub fn type_pair(
        self,
        rust_type: impl AsRef<str>,
        wire_type: impl AsRef<str>,
    ) -> TypePairBuilder {
        self.finish().type_pair(rust_type, wire_type)
    }

    /// Return to the owning [`TypeRegistry`].
    pub fn finish(self) -> TypeRegistry {
        self.registry
    }
}

impl TypeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add or replace a Rust/Wire type pair.
    pub fn type_pair(
        mut self,
        rust_type: impl AsRef<str>,
        wire_type: impl AsRef<str>,
    ) -> TypePairBuilder {
        let rust_type = rust_type.as_ref().to_owned();
        self.add_type_pair_mut(&rust_type, wire_type);
        TypePairBuilder {
            registry: self,
            rust_type,
        }
    }

    /// Add or replace the input conversion function for an already
    /// registered Rust type.
    pub fn add_input_conversion_function(
        mut self,
        rust_type: impl AsRef<str>,
        decode: InlineFn,
    ) -> Self {
        self.add_input_conversion_function_mut(rust_type, decode);
        self
    }

    /// Add or replace the output conversion function for an already
    /// registered Rust type.
    pub fn add_output_conversion_function(
        mut self,
        rust_type: impl AsRef<str>,
        encode: InlineFn,
    ) -> Self {
        self.add_output_conversion_function_mut(rust_type, encode);
        self
    }

    /// Merge another registry into this one. Entries in `other` override
    /// entries with the same key in `self`.
    pub fn merge(mut self, other: TypeRegistry) -> Self {
        self.types.extend(other.types);
        self
    }

    pub(crate) fn add_type_pair_mut(
        &mut self,
        rust_type: impl AsRef<str>,
        wire_type: impl AsRef<str>,
    ) {
        let key = canon_type(rust_type.as_ref());
        let parsed_wire = syn::parse_str::<syn::Type>(wire_type.as_ref())
            .unwrap_or_else(|e| panic!("invalid wire type `{}`: {}", wire_type.as_ref(), e));

        match self.types.get_mut(&key) {
            Some(binding) => binding.wire_type = parsed_wire,
            None => {
                self.types.insert(
                    key,
                    TypeBinding::input_output(rust_type, parsed_wire, None, None),
                );
            }
        }
    }

    pub(crate) fn add_input_conversion_function_mut(
        &mut self,
        rust_type: impl AsRef<str>,
        decode: InlineFn,
    ) {
        let key = canon_type(rust_type.as_ref());
        let binding = self.types.get_mut(&key).unwrap_or_else(|| {
            panic!(
                "missing type pair for `{}`: call add_type_pair first",
                rust_type.as_ref()
            )
        });
        binding.decode = Some(decode);
    }

    pub(crate) fn add_output_conversion_function_mut(
        &mut self,
        rust_type: impl AsRef<str>,
        encode: InlineFn,
    ) {
        let key = canon_type(rust_type.as_ref());
        let binding = self.types.get_mut(&key).unwrap_or_else(|| {
            panic!(
                "missing type pair for `{}`: call add_type_pair first",
                rust_type.as_ref()
            )
        });
        binding.encode = Some(encode);
    }

    /// Internal: drain the entries of another registry into this one.
    /// Used by builder fluent methods that take `TypeRegistry` by value.
    pub(crate) fn extend_from(&mut self, other: TypeRegistry) {
        self.types.extend(other.types);
    }
}

/// Pre-built registry containing universal language-primitive rows
/// (`bool`, `i64`, `f64`). These have JNI-shaped wire forms today; if a
/// non-JNI destination is added, callers should construct their own
/// builtins set rather than relying on this one.
///
/// Kept here as a free function so the universal core has no opinion
/// about which primitives are pre-registered.
pub fn primitive_builtins() -> TypeRegistry {
    TypeRegistry::new()
        .type_pair("bool", "jni::sys::jboolean")
        .input(
            InlineFn::new(|input: Option<&syn::Ident>| -> TokenStream {
                let input = input.expect("bool decode requires an input ident");
                quote! { #input != 0 }
            }),
        )
        .type_pair("i64", "jni::sys::jlong")
        .input(
            InlineFn::new(|input: Option<&syn::Ident>| -> TokenStream {
                let input = input.expect("i64 decode requires an input ident");
                quote! { #input }
            }),
        )
        .type_pair("f64", "jni::sys::jdouble")
        .input(
            InlineFn::new(|input: Option<&syn::Ident>| -> TokenStream {
                let input = input.expect("f64 decode requires an input ident");
                quote! { #input }
            }),
        )
        .type_pair("Duration", "jni::sys::jlong")
        .input(
            InlineFn::new(|input: Option<&syn::Ident>| -> TokenStream {
                let input = input.expect("Duration decode requires an input ident");
                quote! { std::time::Duration::from_millis(#input as u64) }
            }),
        )
        .finish()
}
