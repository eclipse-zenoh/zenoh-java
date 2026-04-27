//! Type-specific constructors for JNI param-direction [`TypeBinding`] rows.

use crate::core::type_binding::TypeBinding;
use crate::jni::inline_fn_helpers::{env_ref_decode, env_ref_mut_decode, pure_decode};
use crate::jni::jni_type;

/// `JObject` wire shape using `&mut env` decode style.
pub fn jobject(rust_type: impl AsRef<str>, decode: impl AsRef<str>) -> TypeBinding {
    TypeBinding::input(rust_type, jni_type::jobject(), env_ref_mut_decode(decode))
}

/// `JString` wire shape using `&mut env` decode style.
pub fn jstring(rust_type: impl AsRef<str>, decode: impl AsRef<str>) -> TypeBinding {
    TypeBinding::input(rust_type, jni_type::jstring(), env_ref_mut_decode(decode))
}

/// `JByteArray` wire shape using `&env` decode style.
pub fn jbyte_array(rust_type: impl AsRef<str>, decode: impl AsRef<str>) -> TypeBinding {
    TypeBinding::input(rust_type, jni_type::jbyte_array(), env_ref_decode(decode))
}

/// `jint` wire shape using pure decode style.
pub fn jint(rust_type: impl AsRef<str>, decode: impl AsRef<str>) -> TypeBinding {
    TypeBinding::input(rust_type, jni_type::jint(), pure_decode(decode))
}