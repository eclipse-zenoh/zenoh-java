//! Universal collection of type bindings, keyed by the canonical
//! `to_token_stream()` form of the Rust type-shape.

use std::collections::HashMap;
use std::sync::Arc;

use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};

use crate::core::inline_fn::{input_fn, output_fn, InputFn, OutputFn, NO_INPUT, NO_OUTPUT};
use crate::core::type_binding::{canon_type, TypeBinding};

#[derive(Clone)]
struct WrapPattern {
    prefix: String,
    suffix: String,
    wrap_input: Arc<dyn Fn(&TypeBinding) -> InputFn + Send + Sync>,
    wrap_output: Arc<dyn Fn(&TypeBinding) -> OutputFn + Send + Sync>,
    wrap_wire: Arc<dyn Fn(&TypeBinding) -> syn::Type + Send + Sync>,
}

fn find_wildcard(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'_' {
            let before_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_';
            let after_ok = i + 1 >= bytes.len()
                || !bytes[i + 1].is_ascii_alphanumeric() && bytes[i + 1] != b'_';
            if before_ok && after_ok {
                return Some(i);
            }
        }
    }
    None
}

#[derive(Default, Clone)]
pub struct TypeRegistry {
    pub(crate) types: HashMap<String, TypeBinding>,
    wrap_patterns: Vec<WrapPattern>,
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

    /// Register a wildcard wrapper pattern.
    ///
    /// `pattern` must contain exactly one `_` wildcard, e.g. `"Option<_>"`.
    /// `wrap_input` and `wrap_output` receive the full inner [`TypeBinding`]
    /// (giving access to the inner wire type) and return the outer conversion
    /// functions. The outer wire type is inherited from the inner binding;
    /// use [`wrap_type_wire`](Self::wrap_type_wire) when it must differ.
    ///
    /// Exact registrations always win over wildcard synthesis.
    pub fn wrap_type(
        self,
        pattern: impl AsRef<str>,
        wrap_input: impl Fn(&TypeBinding) -> InputFn + Send + Sync + 'static,
        wrap_output: impl Fn(&TypeBinding) -> OutputFn + Send + Sync + 'static,
    ) -> Self {
        self.wrap_type_wire(pattern, wrap_input, wrap_output, |inner| {
            inner.wire_type.clone()
        })
    }

    /// Like [`wrap_type`](Self::wrap_type) but also takes `wrap_wire` to
    /// override the outer binding's wire type. Use this when the wrapped type
    /// has a different wire form than the inner type — e.g. `Option<bool>`
    /// uses `JObject` (boxed `Boolean`) rather than inheriting `jboolean`.
    pub fn wrap_type_wire(
        mut self,
        pattern: impl AsRef<str>,
        wrap_input: impl Fn(&TypeBinding) -> InputFn + Send + Sync + 'static,
        wrap_output: impl Fn(&TypeBinding) -> OutputFn + Send + Sync + 'static,
        wrap_wire: impl Fn(&TypeBinding) -> syn::Type + Send + Sync + 'static,
    ) -> Self {
        let canonical = canon_type(pattern.as_ref());
        let idx = find_wildcard(&canonical).unwrap_or_else(|| {
            panic!(
                "wrap_type pattern `{}` must contain a standalone `_` wildcard",
                pattern.as_ref()
            )
        });
        self.wrap_patterns.push(WrapPattern {
            prefix: canonical[..idx].to_string(),
            suffix: canonical[idx + 1..].to_string(),
            wrap_input: Arc::new(wrap_input),
            wrap_output: Arc::new(wrap_output),
            wrap_wire: Arc::new(wrap_wire),
        });
        self
    }

    /// Look up a type binding by its canonical key.
    ///
    /// Tries an exact match first; if none is found, iterates wildcard
    /// patterns registered via [`wrap_type`](Self::wrap_type). For a
    /// matching pattern the inner type is looked up (exact only) and a new
    /// [`TypeBinding`] is synthesised on the fly — wire type inherited from
    /// the inner binding, conversion functions wrapped.
    pub(crate) fn get_binding(&self, key: &str) -> Option<TypeBinding> {
        if let Some(b) = self.types.get(key) {
            return Some(b.clone());
        }
        for pattern in &self.wrap_patterns {
            if key.starts_with(&pattern.prefix) && key.ends_with(&pattern.suffix) {
                let inner_key = key[pattern.prefix.len()..key.len() - pattern.suffix.len()].trim();
                if let Some(inner) = self.types.get(inner_key) {
                    let rust_type = syn::parse_str::<syn::Type>(key)
                        .unwrap_or_else(|e| panic!("wrap_type: cannot parse `{}`: {}", key, e));
                    let wire_type = (pattern.wrap_wire)(inner);
                    let decode = (pattern.wrap_input)(inner);
                    let encode = (pattern.wrap_output)(inner);
                    return Some(TypeBinding::input_output(rust_type, wire_type, decode, encode));
                }
            }
        }
        None
    }

    /// Merge another registry into this one. Entries in `other` override
    /// entries with the same key in `self`.
    pub fn merge(mut self, other: TypeRegistry) -> Self {
        self.types.extend(other.types);
        self.wrap_patterns.extend(other.wrap_patterns);
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
        self.wrap_patterns.extend(other.wrap_patterns);
    }
}

/// Input conversion function for `bool`: converts non-zero to true.
fn bool_input(input: &syn::Ident) -> TokenStream {
    quote! { #input != 0 }
}

/// Identity conversion function for `T`: returns the input as-is.
fn id_input(input: &syn::Ident) -> TokenStream {
    quote! { #input }
}

/// Output encoder for `bool`: deref + cast `&bool` to `jboolean`.
/// Encoders are always called with a borrowed input (`&value.field` from
/// the struct encoder, `&__arg<i>` from the callback strategy).
fn bool_output(output: Option<&syn::Ident>) -> TokenStream {
    match output {
        Some(o) => quote! { (*(#o)) as jni::sys::jboolean },
        None => quote! { 0 as jni::sys::jboolean },
    }
}

/// Output encoder for `i64`: deref + cast `&i64` to `jlong`.
fn i64_output(output: Option<&syn::Ident>) -> TokenStream {
    match output {
        Some(o) => quote! { (*(#o)) as jni::sys::jlong },
        None => quote! { 0 as jni::sys::jlong },
    }
}

/// Output encoder for `f64`: deref + cast `&f64` to `jdouble`.
fn f64_output(output: Option<&syn::Ident>) -> TokenStream {
    match output {
        Some(o) => quote! { (*(#o)) as jni::sys::jdouble },
        None => quote! { 0.0 as jni::sys::jdouble },
    }
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

/// Returns `true` if `ty` is a JNI primitive type (`jboolean`, `jlong`, …).
///
/// JNI primitives live in `jni::sys` and have all-lowercase names starting
/// with `j`; JNI reference types (`JObject`, `JString`, …) start with an
/// uppercase `J`.
fn is_jni_primitive(ty: &syn::Type) -> bool {
    if let syn::Type::Path(tp) = ty {
        if let Some(last) = tp.path.segments.last() {
            let name = last.ident.to_string();
            return name.starts_with('j')
                && name.len() > 1
                && name.chars().nth(1).is_some_and(|c| c.is_ascii_lowercase());
        }
    }
    false
}

/// Boxing metadata for a JNI primitive: (Java class path, valueOf descriptor,
/// JValue variant name, unbox method name, unbox descriptor, JValueOwned getter).
fn jni_box_info(type_name: &str) -> (&'static str, &'static str, &'static str, &'static str, &'static str, &'static str) {
    match type_name {
        "jboolean" => ("java/lang/Boolean",   "(Z)Ljava/lang/Boolean;",   "Bool",   "booleanValue", "()Z", "z"),
        "jbyte"    => ("java/lang/Byte",      "(B)Ljava/lang/Byte;",      "Byte",   "byteValue",    "()B", "b"),
        "jchar"    => ("java/lang/Character", "(C)Ljava/lang/Character;", "Char",   "charValue",    "()C", "c"),
        "jshort"   => ("java/lang/Short",     "(S)Ljava/lang/Short;",     "Short",  "shortValue",   "()S", "s"),
        "jint"     => ("java/lang/Integer",   "(I)Ljava/lang/Integer;",   "Int",    "intValue",     "()I", "i"),
        "jlong"    => ("java/lang/Long",      "(J)Ljava/lang/Long;",      "Long",   "longValue",    "()J", "j"),
        "jfloat"   => ("java/lang/Float",     "(F)Ljava/lang/Float;",     "Float",  "floatValue",   "()F", "f"),
        "jdouble"  => ("java/lang/Double",    "(D)Ljava/lang/Double;",    "Double", "doubleValue",  "()D", "d"),
        other => panic!("jni_box_info: not a known JNI primitive type `{}`", other),
    }
}

/// Wraps an [`InputFn`] (or anything that converts into one) for `T` into an
/// [`InputFn`] for `Option<T>`.
///
/// The wire value must expose an `.is_null()` method (JNI reference types);
/// a truthy result maps to `None`, otherwise the inner conversion is applied.
/// For primitive inner types use [`input_option`] instead.
pub fn nullable_to_option(inner: impl Into<InputFn>) -> InputFn {
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
/// The `None` arm of the inner function is reused as the null wire value.
/// For primitive inner types use [`output_option`] instead.
pub fn option_to_nullable(inner: impl Into<OutputFn>) -> OutputFn {
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

/// Decoder for `Option<T>` that handles both JNI reference types (checked via
/// `.is_null()`) and JNI primitive types (unboxed through their Java object
/// wrappers, e.g. `Boolean`, `Long`).
///
/// For reference inner types the generated code is identical to
/// [`nullable_to_option`]. For primitive inner types (`jboolean`, `jlong`, …)
/// the incoming wire value is a nullable `JObject` holding the boxed primitive;
/// unboxing is done via the appropriate Java method (`booleanValue`, etc.).
pub fn input_option(inner: &TypeBinding) -> InputFn {
    if is_jni_primitive(&inner.wire_type) {
        let type_name = if let syn::Type::Path(tp) = &inner.wire_type {
            tp.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default()
        } else {
            String::new()
        };
        let (_, _, _, unbox_method, unbox_sig, jvalue_getter) = jni_box_info(&type_name);
        let wire_ty_str = {
            use quote::ToTokens as _;
            inner.wire_type.to_token_stream().to_string()
        };
        let inner_decode = inner.decode.clone();
        InputFn::new(move |input: &syn::Ident| -> TokenStream {
            let wire_ty: syn::Type = syn::parse_str(&wire_ty_str).unwrap();
            let inner_ident = syn::Ident::new("__unboxed", Span::call_site());
            let jvalue_getter_ident = syn::Ident::new(jvalue_getter, Span::call_site());
            let inner_expr = inner_decode.call(&inner_ident);
            quote! {
                if !#input.is_null() {
                    let #inner_ident: #wire_ty = env
                        .call_method(&#input, #unbox_method, #unbox_sig, &[])
                        .and_then(|v| v.#jvalue_getter_ident())
                        .map_err(|err| zerror!(err))?;
                    Some(#inner_expr)
                } else {
                    None
                }
            }
        })
    } else {
        nullable_to_option(inner.decode.clone())
    }
}

/// Encoder for `Option<T>` that handles both JNI reference types and JNI
/// primitive types (boxed via their Java object wrappers).
///
/// For reference inner types the generated code is identical to
/// [`option_to_nullable`]. For primitive inner types (`jboolean`, `jlong`, …)
/// `None` maps to `JObject::null()` and `Some(v)` boxes the primitive via the
/// appropriate Java static factory (`Boolean.valueOf`, `Long.valueOf`, …).
pub fn output_option(inner: &TypeBinding) -> OutputFn {
    if is_jni_primitive(&inner.wire_type) {
        let type_name = if let syn::Type::Path(tp) = &inner.wire_type {
            tp.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default()
        } else {
            String::new()
        };
        let (java_class, box_sig, jvalue_variant, _, _, _) = jni_box_info(&type_name);
        let wire_ty_str = {
            use quote::ToTokens as _;
            inner.wire_type.to_token_stream().to_string()
        };
        let inner_encode = inner.encode.clone();
        OutputFn::new(move |output: Option<&syn::Ident>| -> TokenStream {
            let wire_ty: syn::Type = syn::parse_str(&wire_ty_str).unwrap();
            let jvalue_variant_ident = syn::Ident::new(jvalue_variant, Span::call_site());
            match output {
                None => quote! { jni::objects::JObject::null() },
                Some(output) => {
                    let value_ident = syn::Ident::new("value", Span::call_site());
                    let inner_expr = inner_encode.call(Some(&value_ident));
                    quote! {
                        match &#output {
                            Some(value) => {
                                let __raw: #wire_ty = #inner_expr;
                                env.call_static_method(
                                    #java_class,
                                    "valueOf",
                                    #box_sig,
                                    &[jni::objects::JValue::#jvalue_variant_ident(__raw)],
                                )
                                .and_then(|v| v.l())
                                .map_err(|err| zerror!(err))?
                            }
                            None => jni::objects::JObject::null(),
                        }
                    }
                }
            }
        })
    } else {
        option_to_nullable(inner.encode.clone())
    }
}

/// Wire type for `Option<T>`: returns `JObject` for primitive inner types
/// (since primitives must be boxed to be nullable) and inherits the inner
/// wire type for JNI reference types (which are already nullable).
pub fn option_wire_type(inner: &TypeBinding) -> syn::Type {
    if is_jni_primitive(&inner.wire_type) {
        syn::parse_str("jni::objects::JObject").expect("JObject is a valid type")
    } else {
        inner.wire_type.clone()
    }
}

/// Input function for `ZResult<T>` — always `NO_INPUT`.
///
/// `ZResult<T>` is only ever a return type, never a function parameter.
pub fn input_result(_inner: &TypeBinding) -> InputFn {
    NO_INPUT
}

/// Output function for `ZResult<T>` — delegates to the inner type's encoder.
///
/// The body strategy's `?` operator on the wrapped-fn call already unwraps
/// `ZResult<T>` to `T`, so the encoder receives the unwrapped value directly.
pub fn output_result(inner: &TypeBinding) -> OutputFn {
    inner.encode.clone()
}

/// Wire type for `ZResult<T>` — same as `T`'s wire type.
pub fn result_wire_type(inner: &TypeBinding) -> syn::Type {
    inner.wire_type.clone()
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
        // Strings
        .type_pair(
            "String",
            "jni::objects::JString",
            string_input,
            string_output,
        )
        .type_pair(
            "Vec<u8>",
            "jni::objects::JByteArray",
            bytes_input,
            bytes_output,
        )
        // Primitives — identity-cast encoders make these usable as
        // callback args and as fields of auto-encoded structs.
        .type_pair("bool", "jni::sys::jboolean", bool_input, bool_output)
        .type_pair("i64", "jni::sys::jlong", id_input, i64_output)
        .type_pair("f64", "jni::sys::jdouble", id_input, f64_output)
        .type_pair("Duration", "jni::sys::jlong", duration_input, NO_OUTPUT)
        .wrap_type_wire("Option<_>", input_option, output_option, option_wire_type)
}
