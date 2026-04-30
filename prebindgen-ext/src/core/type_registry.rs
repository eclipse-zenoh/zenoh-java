//! Universal collection of type bindings, keyed by the canonical
//! `to_token_stream()` form of the Rust type-shape.

use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};

use crate::core::inline_fn::{option_input, option_output, InputFn, OutputFn, NO_OUTPUT};
use crate::core::type_binding::{canon_type, TypeBinding};

#[derive(Default, Clone)]
pub struct TypeRegistry {
    pub(crate) types: HashMap<String, TypeBinding>,
}

impl TypeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add or replace a Rust/Wire type pair together with conversion
    /// functions used for wire-to-Rust (`input`) and Rust-to-wire (`output`).
    pub fn type_pair(
        mut self,
        rust_type: impl AsRef<str>,
        wire_type: impl AsRef<str>,
        input: InputFn,
        output: OutputFn,
    ) -> Self {
        let rust_type = rust_type.as_ref();
        self.add_type_pair_mut(rust_type, wire_type);
        self.add_input_conversion_function_mut(rust_type, input);
        self.add_output_conversion_function_mut(rust_type, output);
        self
    }

    /// Add or replace the input conversion function for an already
    /// registered Rust type.
    pub fn add_input_conversion_function(
        mut self,
        rust_type: impl AsRef<str>,
        decode: InputFn,
    ) -> Self {
        self.add_input_conversion_function_mut(rust_type, decode);
        self
    }

    /// Add or replace the output conversion function for an already
    /// registered Rust type.
    pub fn add_output_conversion_function(
        mut self,
        rust_type: impl AsRef<str>,
        encode: OutputFn,
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
        let parsed_rust = syn::parse_str::<syn::Type>(rust_type.as_ref())
            .unwrap_or_else(|e| panic!("invalid rust type `{}`: {}", rust_type.as_ref(), e));
        let key = parsed_rust.to_token_stream().to_string();
        let parsed_wire = syn::parse_str::<syn::Type>(wire_type.as_ref())
            .unwrap_or_else(|e| panic!("invalid wire type `{}`: {}", wire_type.as_ref(), e));

        match self.types.get_mut(&key) {
            Some(binding) => binding.wire_type = parsed_wire,
            None => {
                self.types.insert(
                    key,
                    TypeBinding::input_output(parsed_rust, parsed_wire, None, None),
                );
            }
        }
    }

    pub(crate) fn add_input_conversion_function_mut(
        &mut self,
        rust_type: impl AsRef<str>,
        decode: InputFn,
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
        encode: OutputFn,
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
    let bool_input = InputFn::new(|input: &syn::Ident| -> TokenStream {
        quote! { #input != 0 }
    });
    let id_input = InputFn::new(|input: &syn::Ident| -> TokenStream {
        quote! { #input }
    });
    let duration_input = InputFn::new(|input: &syn::Ident| -> TokenStream {
        quote! { std::time::Duration::from_millis(#input as u64) }
    });
    let string_input = InputFn::new(|input: &syn::Ident| -> TokenStream {
        quote! {
            zenoh_flat::jni::decode_string(&mut env, &#input)
                .map_err(|err| zerror!(err))?
        }
    });
    let string_output = OutputFn::new(|output: Option<&syn::Ident>| -> TokenStream {
        match output {
            Some(output) => quote! {
                zenoh_flat::jni::encode_string(&mut env, #output)
                    .map_err(|err| zerror!(err))?
            },
            None => quote! { zenoh_flat::jni::null_string() },
        }
    });
    let string_option_input = option_input(string_input.clone());
    let string_option_output = option_output(string_output.clone());
    let bytes_input = InputFn::new(|input: &syn::Ident| -> TokenStream {
        quote! {
            zenoh_flat::jni::decode_byte_array(&mut env, &#input)
                .map_err(|err| zerror!(err))?
        }
    });
    let bytes_option_input = option_input(bytes_input.clone());
    let bytes_output = OutputFn::new(|output: Option<&syn::Ident>| -> TokenStream {
        match output {
            Some(output) => quote! {
                zenoh_flat::jni::encode_byte_array(&mut env, #output)
                    .map_err(|err| zerror!(err))?
            },
            None => quote! { zenoh_flat::jni::null_byte_array() },
        }
    });
    let bytes_option_output = option_output(bytes_output.clone());

    TypeRegistry::new()
        // Strings
        .type_pair(
            "String",
            "jni::objects::JString",
            string_input,
            string_output,
        )
        .type_pair(
            "Option<String>",
            "jni::objects::JString",
            string_option_input,
            string_option_output,
        )
        .type_pair(
            "Vec<u8>",
            "jni::objects::JByteArray",
            bytes_input,
            bytes_output,
        )
        .type_pair(
            "Option<Vec<u8>>",
            "jni::objects::JByteArray",
            bytes_option_input,
            bytes_option_output,
        )
        // Primitives
        .type_pair("bool", "jni::sys::jboolean", bool_input, NO_OUTPUT)
        .type_pair("i64", "jni::sys::jlong", id_input.clone(), NO_OUTPUT)
        .type_pair("f64", "jni::sys::jdouble", id_input, NO_OUTPUT)
        .type_pair("Duration", "jni::sys::jlong", duration_input, NO_OUTPUT)
}
