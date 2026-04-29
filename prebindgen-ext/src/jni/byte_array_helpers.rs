//! JNI byte-array conversion helpers for use in type converters and runtime code.

use jni::objects::{JByteArray, JObject};
use jni::JNIEnv;

/// Converts a JNI `JByteArray` into a Rust `Vec<u8>`.
pub fn decode_byte_array(env: &mut JNIEnv, payload: &JByteArray) -> Result<Vec<u8>, String> {
    env.convert_byte_array(payload)
        .map_err(|err| format!("Error while decoding JByteArray: {}", err))
}

/// Converts a Rust byte slice into a JNI `JByteArray`.
pub fn encode_byte_array<'local>(
    env: &mut JNIEnv<'local>,
    bytes: &[u8],
) -> Result<JByteArray<'local>, String> {
    env.byte_array_from_slice(bytes)
        .map_err(|err| format!("Error while encoding JByteArray: {}", err))
}

/// Returns a null JNI byte-array handle.
pub fn null_byte_array() -> JByteArray<'static> {
    JByteArray::from(JObject::null())
}
