//! `StructStrategy` impl that emits a `decode_<StructName>` fn for each
//! `#[prebindgen]` struct and registers a `TypeBinding` so the struct can
//! appear by value in a wrapped function's signature.

use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use proc_macro2::Span;

use prebindgen::SourceLocation;

use crate::core::type_binding::TypeBinding;
use crate::core::type_registry::TypeRegistry;
use crate::core::types_converter::StructStrategy;
use crate::jni::inline_fn_helpers::{env_ref_mut_decode, env_ref_mut_encode};
use crate::jni::jni_type;
use crate::jni::wire_access::jni_field_access;
use crate::util::snake_to_camel;

/// JNI struct decoder strategy.
///
/// For each `#[prebindgen]` struct, emits:
/// ```ignore
/// pub(crate) fn decode_<Name>(
///     env: &mut jni::JNIEnv,
///     obj: &jni::objects::JObject,
/// ) -> <zresult><source_module>::<Name> { ... }
/// ```
/// and registers a `TypeBinding::param("<Name>", jni_type::jobject(),
/// env_ref_mut_decode("decode_<Name>"))` so the struct can be passed by value
/// to a wrapped function.
///
/// Also emits an `encode_<Name>` function if all struct fields have available
/// encoders. The encoder creates a JObject and sets all fields from the Rust struct.
///
/// Field types are resolved by **bare ident** (e.g. `bool`, `i64`,
/// `CongestionControl`), not by canonical token-stream key — Rust struct
/// fields written as `pub field: bool` produce the path `bool`, and that
/// is the lookup key used here.
pub struct JniDecoderStruct {
    pub source_module: syn::Path,
    pub zresult: syn::Path,
    pub zerror_macro: syn::Path,
    pub java_class_prefix: Option<String>,
}

impl JniDecoderStruct {
    /// Build a strategy with the given module and result type, defaulting
    /// the error macro to bare `zerror` (matches the existing zenoh-jni
    /// codebase convention).
    pub fn new(source_module: impl AsRef<str>, zresult: impl AsRef<str>) -> Self {
        Self {
            source_module: syn::parse_str(source_module.as_ref())
                .expect("invalid source_module path"),
            zresult: syn::parse_str(zresult.as_ref()).expect("invalid zresult path"),
            zerror_macro: syn::parse_str("zerror").unwrap(),
            java_class_prefix: None,
        }
    }

    pub fn zerror_macro(mut self, path: impl AsRef<str>) -> Self {
        self.zerror_macro = syn::parse_str(path.as_ref()).expect("invalid zerror_macro path");
        self
    }

    /// Set destination Java package prefix (slash-separated, e.g. `io/zenoh/jni`).
    /// When unset, generated encoders instantiate by simple class name.
    pub fn java_class_prefix(mut self, prefix: impl AsRef<str>) -> Self {
        let p = prefix.as_ref().trim().trim_matches('/').to_string();
        self.java_class_prefix = if p.is_empty() { None } else { Some(p) };
        self
    }
}

impl StructStrategy for JniDecoderStruct {
    fn process(
        &self,
        s: &syn::ItemStruct,
        loc: &SourceLocation,
        registry: &mut TypeRegistry,
        out: &mut Vec<(syn::Item, SourceLocation)>,
    ) {
        let struct_name = s.ident.to_string();
        let struct_ident = s.ident.clone();
        let decoder_ident = format_ident!("decode_{}", struct_ident);
        let encoder_ident = format_ident!("encode_{}", struct_ident);
        let zresult = &self.zresult;
        let struct_module = &self.source_module;
        let zerror = &self.zerror_macro;
        let java_class_name = match &self.java_class_prefix {
            Some(prefix) => format!("{prefix}/{struct_name}"),
            None => struct_name.clone(),
        };

        let syn::Fields::Named(named) = &s.fields else {
            panic!("tuple / unit structs are not supported at {loc}");
        };

        let mut field_preludes: Vec<TokenStream> = Vec::new();
        let mut field_init: Vec<TokenStream> = Vec::new();
        let mut encoder_field_preludes: Vec<TokenStream> = Vec::new();
        let mut ctor_sig = String::from("(");
        let mut ctor_args: Vec<TokenStream> = Vec::new();
        let mut all_fields_have_encoders = true;

        for field in &named.named {
            let fname_ident = field
                .ident
                .as_ref()
                .unwrap_or_else(|| panic!("unnamed field in struct `{struct_name}` at {loc}"))
                .clone();
            let fname = fname_ident.to_string();
            let camel_fname = snake_to_camel(&fname);
            let err_prefix = format!("{struct_name}.{camel_fname}: {{}}");

            let binding = lookup_field_binding(registry, &field.ty).unwrap_or_else(|| {
                panic!(
                    "unsupported field type `{}` for `{}.{}` at {loc}",
                    field.ty.to_token_stream(),
                    struct_name,
                    fname
                )
            });
            let raw_ident = format_ident!("__{}_raw", fname_ident);
            let jni_type = binding.wire_type();
            let decode_expr = binding
                .decode()
                .expect("struct-field binding must have a decode")
                .call(&raw_ident);
            match jni_field_access(binding.wire_type()) {
                Some((jni_sig, jvalue_method, false)) => {
                    field_preludes.push(quote! {
                        let #raw_ident: #jni_type = env.get_field(obj, #camel_fname, #jni_sig)
                            .and_then(|v| v.#jvalue_method())
                            .map_err(|err| #zerror!(#err_prefix, err))? as _;
                        let #fname_ident = #decode_expr;
                    });
                }
                Some((jni_sig, _jvalue_method, true)) => {
                    let tmp_ident = format_ident!("__{}_jobj", fname_ident);
                    field_preludes.push(quote! {
                        let #tmp_ident: jni::objects::JObject = env.get_field(obj, #camel_fname, #jni_sig)
                            .and_then(|v| v.l())
                            .map_err(|err| #zerror!(#err_prefix, err))?;
                        let #raw_ident: #jni_type = #tmp_ident.into();
                        let #fname_ident = #decode_expr;
                    });
                }
                None => {
                    panic!(
                        "field `{}.{}` at {loc}: unsupported JNI wire form `{}`",
                        struct_name,
                        fname,
                        binding.wire_type().to_token_stream()
                    );
                }
            };
            field_init.push(quote! { #fname_ident });

            // Check if encoder is available for this field
            if let Some(encode) = binding.encode().filter(|encode| encode.is_implemented()) {
                let field_ref_ident = format_ident!("__{}_value", fname_ident);
                let encoded_ident = format_ident!("__{}_encoded", fname_ident);
                let encode_expr = encode.call(Some(&field_ref_ident));
                encoder_field_preludes.push(quote! {
                    let #field_ref_ident = &value.#fname_ident;
                    let #encoded_ident = #encode_expr;
                });

                match jni_field_access(binding.wire_type()) {
                    Some((jni_sig, _jvalue_method, false)) => {
                        ctor_sig.push_str(jni_sig);
                        ctor_args.push(quote! {
                            jni::objects::JValue::from(#encoded_ident)
                        });
                    }
                    Some((jni_sig, _jvalue_method, true)) => {
                        let encoded_obj_ident = format_ident!("__{}_encoded_obj", fname_ident);
                        encoder_field_preludes.push(quote! {
                            let #encoded_obj_ident: jni::objects::JObject = #encoded_ident.into();
                        });
                        ctor_sig.push_str(jni_sig);
                        ctor_args.push(quote! {
                            jni::objects::JValue::Object(&#encoded_obj_ident)
                        });
                    }
                    None => {
                        panic!(
                            "field `{}.{}` at {loc}: unsupported JNI wire form for encoding `{}`",
                            struct_name,
                            fname,
                            binding.wire_type().to_token_stream()
                        );
                    }
                }
            } else {
                all_fields_have_encoders = false;
            }
        }

        let decoder_tokens = quote! {
            #[allow(non_snake_case, unused_mut, unused_variables)]
            pub(crate) fn #decoder_ident(
                mut env: &mut jni::JNIEnv,
                obj: &jni::objects::JObject,
            ) -> #zresult<#struct_module::#struct_ident> {
                #(#field_preludes)*
                Ok(#struct_module::#struct_ident {
                    #(#field_init),*
                })
            }
        };

        let decoder_path = format!("decode_{struct_name}");
        registry.add_type_pair_mut(&struct_name, jni_type::jobject());
        registry.add_input_conversion_function_mut(&struct_name, env_ref_mut_decode(&decoder_path));

        let decoder_item: syn::Item = syn::parse2(decoder_tokens).expect("generated struct decoder must parse");
        out.push((decoder_item, loc.clone()));

        // Only generate encoder if all fields have encoders available
        if all_fields_have_encoders {
            ctor_sig.push_str(")V");
            let ctor_sig_lit = syn::LitStr::new(&ctor_sig, Span::call_site());
            let encoder_tokens = quote! {
                #[allow(non_snake_case, unused_mut, unused_variables)]
                pub(crate) fn #encoder_ident(
                    mut env: &mut jni::JNIEnv,
                    value: #struct_module::#struct_ident,
                ) -> #zresult<jni::sys::jobject> {
                    #(#encoder_field_preludes)*
                    let obj = env.new_object(
                        #java_class_name,
                        #ctor_sig_lit,
                        &[#(#ctor_args),*],
                    )
                    .map_err(|err| #zerror!(err))?;
                    Ok(obj.as_raw())
                }
            };

            let encoder_path = format!("encode_{struct_name}");
            registry.add_output_conversion_function_mut(&struct_name, env_ref_mut_encode(&encoder_path));

            let encoder_item: syn::Item = syn::parse2(encoder_tokens).expect("generated struct encoder must parse");
            out.push((encoder_item, loc.clone()));
        }
    }
}

/// Look up a `#[prebindgen]` struct field's type in the registry.
///
/// Fast path: bare path-tail ident (e.g. `bool`, `i64`, `CongestionControl`).
/// Fallback: full canonical token-stream key, which allows generic field types
/// like `Option<ManuallyDrop<Arc<ZKeyExpr<'static'>>>>` to be registered and
/// found by callers that use the full type expression as the registry key.
fn lookup_field_binding<'a>(
    registry: &'a TypeRegistry,
    ty: &syn::Type,
) -> Option<&'a TypeBinding> {
    use quote::ToTokens as _;
    // Fast path: last path segment.
    if let syn::Type::Path(tp) = ty {
        if let Some(last) = tp.path.segments.last() {
            if let Some(b) = registry.types.get(&last.ident.to_string()) {
                return Some(b);
            }
        }
    }
    // Fallback: full canonical token-stream key.
    let key = crate::core::type_binding::canon_type(&ty.to_token_stream().to_string());
    registry.types.get(&key)
}

