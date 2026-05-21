//! Framework-owned exception type for JNI binding failures.
//!
//! Raised when a built-in converter can't honour the JNI shape it was
//! given: UTF-8 decode of a JString, `instanceof` check failure, null
//! JObject where a value was required, struct field read failure, etc.
//! Distinct from any application exception class declared via
//! [`crate::jni::JniExt::kotlin_exception_class`] — application errors
//! still flow through their own throw fns; binding errors land here.
//!
//! `JniExt::new()` pre-registers this type as `exceptions[0]` so it is
//! always available as `throw_JniBindingError(env, &err)` in the
//! generated bindings, and its Kotlin class is auto-emitted under the
//! app's configured package (e.g. `io.zenoh.jni.JniBindingError`).
//!
//! Implements `From<String>` so framework-generated converter bodies
//! can keep their existing `<__JniErr as From<String>>::from(...)`
//! shape — the `__JniErr` alias resolves to this type.

/// Universal binding-failure error type raised by framework-built JNI
/// converters. Carries a single message describing the failure
/// context (e.g. `"Sample.encodingId: …"`, `"Option unbox: …"`).
#[derive(Clone)]
pub struct JniBindingError(pub String);

impl From<String> for JniBindingError {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl core::fmt::Display for JniBindingError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::fmt::Debug for JniBindingError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "JniBindingError({:?})", self.0)
    }
}
