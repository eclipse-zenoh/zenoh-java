//! Universal collection of type bindings, keyed by the canonical
//! `to_token_stream()` form of the Rust type-shape.

use std::collections::HashMap;

use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};

use crate::core::inline_fn::{input_fn, output_fn, InputFn, OutputFn, NO_INPUT, NO_OUTPUT};
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
    ///
    /// Both `input` and `output` accept anything that converts into
    /// [`InputFn`] / [`OutputFn`] — a raw closure, a named function, or a
    /// pre-built `InputFn`/`OutputFn` value.
    pub fn type_pair(
        mut self,
        rust_type: impl AsRef<str>,
        wire_type: impl AsRef<str>,
        input: impl Into<InputFn>,
        output: impl Into<OutputFn>,
    ) -> Self {
        let rust_type = rust_type.as_ref();
        self.add_type_pair_mut(rust_type, wire_type);
        self.add_input_conversion_function_mut(rust_type, input.into());
        self.add_output_conversion_function_mut(rust_type, output.into());
        self
    }

    /// Add or replace a Rust/Wire type pair with pre-built [`InputFn`] and [`OutputFn`].
    /// This is primarily for internal use; prefer [`type_pair`](Self::type_pair) for external callers.
    pub(crate) fn type_pair_internal(
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
        decode: impl Fn(&syn::Ident) -> TokenStream + Send + Sync + 'static,
    ) -> Self {
        self.add_input_conversion_function_mut(rust_type, input_fn(decode));
        self
    }

    /// Add or replace the input conversion function with a pre-built [`InputFn`].
    /// Primarily for internal use; prefer [`add_input_conversion_function`](Self::add_input_conversion_function).
    pub(crate) fn add_input_conversion_function_internal(
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
        encode: impl Fn(Option<&syn::Ident>) -> TokenStream + Send + Sync + 'static,
    ) -> Self {
        self.add_output_conversion_function_mut(rust_type, output_fn(encode));
        self
    }

    /// Add or replace the output conversion function with a pre-built [`OutputFn`].
    /// Primarily for internal use; prefer [`add_output_conversion_function`](Self::add_output_conversion_function).
    pub(crate) fn add_output_conversion_function_internal(
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
                    TypeBinding::input_output(parsed_rust, parsed_wire, NO_INPUT, NO_OUTPUT),
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
        binding.decode = decode;
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
        binding.encode = encode;
    }

    /// Internal: drain the entries of another registry into this one.
    /// Used by builder fluent methods that take `TypeRegistry` by value.
    pub(crate) fn extend_from(&mut self, other: TypeRegistry) {
        self.types.extend(other.types);
    }
}

/// Input conversion function for `bool`: converts non-zero to true.
fn bool_input(input: &syn::Ident) -> TokenStream {
    quote! { #input != 0 }
}

/// Input conversion function for `i64`: identity conversion.
fn id_input(input: &syn::Ident) -> TokenStream {
    quote! { #input }
}

/// Input conversion function for `f64`: identity conversion.
fn f64_input(input: &syn::Ident) -> TokenStream {
    quote! { #input }
}

/// Input conversion function for `Duration`: converts milliseconds to Duration.
fn duration_input(input: &syn::Ident) -> TokenStream {
    quote! { std::time::Duration::from_millis(#input as u64) }
}

/// Input conversion function for `String`: decodes JNI string.
fn string_input(input: &syn::Ident) -> TokenStream {
    quote! {
        zenoh_flat::jni::decode_string(&mut env, &#input)
            .map_err(|err| zerror!(err))?
    }
}

/// Output conversion function for `String`: encodes Rust string to JNI.
fn string_output(output: Option<&syn::Ident>) -> TokenStream {
    match output {
        Some(output) => quote! {
            zenoh_flat::jni::encode_string(&mut env, #output)
                .map_err(|err| zerror!(err))?
        },
        None => quote! { zenoh_flat::jni::null_string() },
    }
}

/// Input conversion function for `Vec<u8>`: decodes JNI byte array.
fn bytes_input(input: &syn::Ident) -> TokenStream {
    quote! {
        zenoh_flat::jni::decode_byte_array(&mut env, &#input)
            .map_err(|err| zerror!(err))?
    }
}

/// Output conversion function for `Vec<u8>`: encodes Rust byte array to JNI.
fn bytes_output(output: Option<&syn::Ident>) -> TokenStream {
    match output {
        Some(output) => quote! {
            zenoh_flat::jni::encode_byte_array(&mut env, #output)
                .map_err(|err| zerror!(err))?
        },
        None => quote! { zenoh_flat::jni::null_byte_array() },
    }
}

/// Output conversion function for types with no output conversion.
fn no_output_fn(_output: Option<&syn::Ident>) -> TokenStream {
    TokenStream::new()
}

/// Wraps an [`InputFn`] (or anything that converts into one) for `T` into an
/// [`InputFn`] for `Option<T>`.
///
/// The wire value must expose an `.is_null()` method (e.g. JNI reference types);
/// a truthy result maps to `None`, otherwise the inner conversion is applied.
pub fn option_input(inner: impl Into<InputFn>) -> InputFn {
    let inner = inner.into();
    InputFn::new(move |input: &syn::Ident| -> TokenStream {
        let inner_expr = inner.call(input);
        quote! {
            if !#input.is_null() {
                Some(#inner_expr)
            } else {
                None
            }
        }
    })
}

/// Wraps an [`OutputFn`] (or anything that converts into one) for `T` into an
/// [`OutputFn`] for `Option<T>`.
///
/// The `None` arm of the inner function is reused as the null wire value,
/// so no separate null-sentinel helper is needed here.
pub fn option_output(inner: impl Into<OutputFn>) -> OutputFn {
    let inner = inner.into();
    OutputFn::new(move |output: Option<&syn::Ident>| -> TokenStream {
        let null_expr = inner.call(None);
        match output {
            Some(output) => {
                let value_ident = syn::Ident::new("value", Span::call_site());
                let inner_expr = inner.call(Some(&value_ident));
                quote! {
                    match &#output {
                        Some(value) => #inner_expr,
                        None => #null_expr,
                    }
                }
            }
            None => null_expr,
        }
    })
}

/// Pre-built registry containing universal language-primitive rows
/// (`bool`, `i64`, `f64`). These have JNI-shaped wire forms today; if a
/// non-JNI destination is added, callers should construct their own
/// builtins set rather than relying on this one.
///
/// Kept here as a free function so the universal core has no opinion
/// about which primitives are pre-registered.
pub fn primitive_builtins() -> TypeRegistry {
    let string_option_input_fn = option_input(string_input);
    let string_option_output_fn = option_output(string_output);
    let bytes_option_input_fn = option_input(bytes_input);
    let bytes_option_output_fn = option_output(bytes_output);

    TypeRegistry::new()
        // Strings
        .type_pair(
            "String",
            "jni::objects::JString",
            string_input,
            string_output,
        )
        .type_pair_internal(
            "Option<String>",
            "jni::objects::JString",
            string_option_input_fn,
            string_option_output_fn,
        )
        .type_pair(
            "Vec<u8>",
            "jni::objects::JByteArray",
            bytes_input,
            bytes_output,
        )
        .type_pair_internal(
            "Option<Vec<u8>>",
            "jni::objects::JByteArray",
            bytes_option_input_fn,
            bytes_option_output_fn,
        )
        // Primitives (no output conversion)
        .type_pair("bool", "jni::sys::jboolean", bool_input, no_output_fn)
        .type_pair("i64", "jni::sys::jlong", id_input, no_output_fn)
        .type_pair("f64", "jni::sys::jdouble", f64_input, no_output_fn)
        .type_pair("Duration", "jni::sys::jlong", duration_input, no_output_fn)
}
