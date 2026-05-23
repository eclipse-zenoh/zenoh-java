//! Map a JNI wire type to its `(jvm_signature_chunk, JValue accessor,
//! is_object)` triple — shared between the struct-strategy decoder/encoder
//! and the callback strategy.

use quote::format_ident;

/// Map a JNI wire type to `(jvm_field_descriptor, JValue_accessor_ident, is_object)`.
///
/// Primitive types (`jlong`, `jint`, …) set `is_object = false` and the
/// accessor names the `.j()` / `.i()` / … `JValue` variant.
///
/// Object types (`JString`, `JByteArray`, …) set `is_object = true`; the
/// caller uses `.l()` to get a `JObject` and then `.into()` to cast to the
/// wire type.
pub(crate) fn jni_field_access(jni_type: &syn::Type) -> Option<(&'static str, syn::Ident, bool)> {
    let syn::Type::Path(tp) = jni_type else {
        return None;
    };
    let last = tp.path.segments.last()?;
    let (sig, accessor, is_obj) = match last.ident.to_string().as_str() {
        "jboolean" => ("Z", "z", false),
        "jbyte" => ("B", "b", false),
        "jchar" => ("C", "c", false),
        "jshort" => ("S", "s", false),
        "jint" => ("I", "i", false),
        "jlong" => ("J", "j", false),
        "jfloat" => ("F", "f", false),
        "jdouble" => ("D", "d", false),
        "JString" => ("Ljava/lang/String;", "l", true),
        "JByteArray" => ("[B", "l", true),
        _ => return None,
    };
    Some((sig, format_ident!("{}", accessor), is_obj))
}
