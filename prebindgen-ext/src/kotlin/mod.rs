//! Kotlin emission support for back-ends that produce Kotlin output.
//!
//! `kotlin_ext` exposes the shared `KotlinFile` / `WriteKotlinError`
//! types; `type_map` is the internal `KotlinTypeMap` (Rust → Kotlin
//! name lookup) consumed by `crate::jni::jni_kotlin_ext`. Both modules
//! are `pub(crate)` — Kotlin emission is JniExt-inherent and not
//! exposed at the crate boundary.

pub(crate) mod kotlin_ext;
pub(crate) mod type_map;
