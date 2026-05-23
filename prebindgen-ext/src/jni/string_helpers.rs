//! JNI String conversion helpers for use in type converters and runtime code.

use jni::objects::{JObject, JString};
use jni::JNIEnv;

/// Converts a JString into a Rust String.
///
/// Used by the String <-> JString converter in primitive_builtins and at runtime.
pub fn decode_string(env: &mut JNIEnv, string: &JString) -> Result<String, String> {
    let binding = env
        .get_string(string)
        .map_err(|err| format!("Error while retrieving JString: {}", err))?;
    let value = binding
        .to_str()
        .map_err(|err| format!("Error decoding JString: {}", err))?;
    Ok(value.to_string())
}

/// Converts a Rust string-like value into a JNI `JString`.
pub fn encode_string<'local, S: AsRef<str>>(
    env: &mut JNIEnv<'local>,
    string: S,
) -> Result<JString<'local>, String> {
    env.new_string(string.as_ref())
        .map_err(|err| format!("Error while encoding JString: {}", err))
}

/// Returns a null JNI string handle.
pub fn null_string() -> JString<'static> {
    JString::from(JObject::null())
}
