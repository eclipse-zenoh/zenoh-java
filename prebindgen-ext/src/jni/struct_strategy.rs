//! `StructStrategy` impl that emits a `decode_<StructName>` fn for each
//! `#[prebindgen]` struct and registers a `TypeBinding` so the struct can
//! appear by value in a wrapped function's signature.

use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};

use prebindgen::SourceLocation;

use crate::core::type_binding::TypeBinding;
use crate::core::type_registry::TypeRegistry;
use crate::core::types_converter::StructStrategy;
use crate::jni::inline_fn_helpers::env_ref_mut_decode;
use crate::jni::jni_type;
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
/// Field types are resolved by **bare ident** (e.g. `bool`, `i64`,
/// `CongestionControl`), not by canonical token-stream key — Rust struct
/// fields written as `pub field: bool` produce the path `bool`, and that
/// is the lookup key used here.
pub struct JniDecoderStruct {
    pub source_module: syn::Path,
    pub zresult: syn::Path,
    pub zerror_macro: syn::Path,
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
        }
    }

    pub fn zerror_macro(mut self, path: impl AsRef<str>) -> Self {
        self.zerror_macro = syn::parse_str(path.as_ref()).expect("invalid zerror_macro path");
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
        let zresult = &self.zresult;
        let struct_module = &self.source_module;
        let zerror = &self.zerror_macro;

        let syn::Fields::Named(named) = &s.fields else {
            panic!("tuple / unit structs are not supported at {loc}");
        };

        let mut field_preludes: Vec<TokenStream> = Vec::new();
        let mut field_init: Vec<TokenStream> = Vec::new();

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
            let (jni_sig, jvalue_method) =
                jni_primitive_signature(binding.wire_type()).unwrap_or_else(|| {
                    panic!(
                        "field `{}.{}` at {loc}: type `{}` has non-primitive JNI wire form `{}`",
                        struct_name,
                        fname,
                        field.ty.to_token_stream(),
                        binding.wire_type().to_token_stream()
                    )
                });
            let raw_ident = format_ident!("__{}_raw", fname_ident);
            let jni_type = binding.wire_type();
            let decode_expr = binding
                .decode()
                .expect("struct-field binding must have a decode")
                .call(Some(&raw_ident));
            field_preludes.push(quote! {
                let #raw_ident: #jni_type = env.get_field(obj, #camel_fname, #jni_sig)
                    .and_then(|v| v.#jvalue_method())
                    .map_err(|err| #zerror!(#err_prefix, err))? as _;
                let #fname_ident = #decode_expr;
            });
            field_init.push(quote! { #fname_ident });
        }

        let tokens = quote! {
            #[allow(non_snake_case, unused_mut, unused_variables)]
            pub(crate) fn #decoder_ident(
                env: &mut jni::JNIEnv,
                obj: &jni::objects::JObject,
            ) -> #zresult<#struct_module::#struct_ident> {
                #(#field_preludes)*
                Ok(#struct_module::#struct_ident {
                    #(#field_init),*
                })
            }
        };

        let decoder_path = format!("decode_{struct_name}");
        let row = TypeBinding::input(
            &struct_name,
            jni_type::jobject(),
            env_ref_mut_decode(&decoder_path),
        );
        registry.insert_raw(row.name().to_string(), row);

        let item: syn::Item = syn::parse2(tokens).expect("generated struct decoder must parse");
        out.push((item, loc.clone()));
    }
}

/// Look up a `#[prebindgen]` struct field's type in the registry. Fields
/// must use the type's bare path-tail name (e.g. `bool`, `i64`,
/// `CongestionControl`) and must resolve to a registered binding.
fn lookup_field_binding<'a>(
    registry: &'a TypeRegistry,
    ty: &syn::Type,
) -> Option<&'a TypeBinding> {
    let syn::Type::Path(tp) = ty else { return None };
    let last = tp.path.segments.last()?;
    let name = last.ident.to_string();
    registry.types.get(&name)
}

/// Map a primitive JNI wire type (`jni::sys::j*`) to the JVM field
/// signature character and the matching `JValue` accessor method.
/// Returns `None` for non-primitive (object-shaped) wire types.
fn jni_primitive_signature(jni_type: &syn::Type) -> Option<(&'static str, syn::Ident)> {
    let syn::Type::Path(tp) = jni_type else {
        return None;
    };
    let last = tp.path.segments.last()?;
    let (sig, accessor) = match last.ident.to_string().as_str() {
        "jboolean" => ("Z", "z"),
        "jbyte" => ("B", "b"),
        "jchar" => ("C", "c"),
        "jshort" => ("S", "s"),
        "jint" => ("I", "i"),
        "jlong" => ("J", "j"),
        "jfloat" => ("F", "f"),
        "jdouble" => ("D", "d"),
        _ => return None,
    };
    Some((sig, format_ident!("{}", accessor)))
}
