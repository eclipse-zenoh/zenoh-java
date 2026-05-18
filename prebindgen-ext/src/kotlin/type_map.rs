//! Per-type Rust → Kotlin name mapping. Internal helper consumed by the
//! Kotlin emitters in `jni_kotlin_ext.rs`; not part of the public API.

use std::collections::HashMap;

use quote::ToTokens;

/// Whitespace-normalise a Rust type string by parsing it as a
/// `syn::Type` and re-serialising. Ensures lookup keys match the
/// canonical form used elsewhere in the pipeline. Falls back to the
/// raw input if parse fails (e.g. legacy `&Session` patterns).
pub(crate) fn canon_type(s: &str) -> String {
    match syn::parse_str::<syn::Type>(s) {
        Ok(ty) => ty.to_token_stream().to_string(),
        Err(_) => s.to_string(),
    }
}

/// Mapping from canonical Rust type-shape to its Kotlin parameter /
/// return type. Values may be either bare Kotlin names (`"Boolean"`,
/// `"String"`) or fully-qualified paths (`"io.zenoh.jni.JNIKeyExpr"`);
/// the generator emits the short name and adds the matching `import`
/// for FQN-shaped values.
#[derive(Default, Clone)]
pub(crate) struct KotlinTypeMap {
    pub(crate) map: HashMap<String, String>,
}

impl KotlinTypeMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(mut self, rust_type: impl AsRef<str>, kotlin_type: impl Into<String>) -> Self {
        self.map
            .insert(canon_type(rust_type.as_ref()), kotlin_type.into());
        self
    }

    pub fn lookup(&self, rust_type: &str) -> Option<&str> {
        self.map.get(&canon_type(rust_type)).map(String::as_str)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> + '_ {
        self.map.iter()
    }

    /// Pre-fill primitive language types whose Kotlin name is fixed.
    pub fn with_primitive_builtins(mut self) -> Self {
        self.map.insert(canon_type("bool"), "Boolean".into());
        self.map.insert(canon_type("i64"), "Long".into());
        self.map.insert(canon_type("f64"), "Double".into());
        self.map.insert(canon_type("Duration"), "Long".into());
        self
    }
}
