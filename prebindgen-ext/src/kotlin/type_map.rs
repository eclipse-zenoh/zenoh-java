//! Per-type Rust → Kotlin name mapping. Decoupled from the universal
//! `TypeBinding` so the core converters stay language-agnostic.

use std::collections::HashMap;

use crate::core::type_binding::canon_type;

/// Mapping from canonical Rust type-shape (the same key form used by
/// [`TypeRegistry`]) to its Kotlin parameter / return type. Values may be
/// either bare Kotlin names (`"Boolean"`, `"String"`) or fully-qualified
/// paths (`"io.zenoh.jni.JNIKeyExpr"`); the generator emits the short
/// name and adds the matching `import` for FQN-shaped values.
#[derive(Default, Clone)]
pub struct KotlinTypeMap {
    pub(crate) map: HashMap<String, String>,
}

impl KotlinTypeMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `rust_type` → `kotlin_type`. The Rust key is canonicalized
    /// via `syn::Type` parse so callers can pass either spacing form.
    pub fn add(mut self, rust_type: impl AsRef<str>, kotlin_type: impl Into<String>) -> Self {
        self.map
            .insert(canon_type(rust_type.as_ref()), kotlin_type.into());
        self
    }

    /// Look up the Kotlin name for a Rust type-shape.
    pub fn lookup(&self, rust_type: &str) -> Option<&str> {
        self.map.get(&canon_type(rust_type)).map(String::as_str)
    }

    /// Pre-fill primitive language types whose Kotlin name is fixed:
    /// `bool→Boolean`, `i64→Long`, `f64→Double`, `Duration→Long`.
    pub fn with_primitive_builtins(mut self) -> Self {
        self.map.insert(canon_type("bool"), "Boolean".into());
        self.map.insert(canon_type("i64"), "Long".into());
        self.map.insert(canon_type("f64"), "Double".into());
        self.map.insert(canon_type("Duration"), "Long".into());
        self
    }
}
