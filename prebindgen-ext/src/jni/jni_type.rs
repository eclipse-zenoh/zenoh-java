//! Small helpers for common JNI wire-type spellings used by generators.

pub fn jboolean() -> &'static str {
    "jni::sys::jboolean"
}

pub fn jbyte_array() -> &'static str {
    "jni::objects::JByteArray"
}

pub fn jclass() -> &'static str {
    "jni::objects::JClass"
}

pub fn jdouble() -> &'static str {
    "jni::sys::jdouble"
}

pub fn jint() -> &'static str {
    "jni::sys::jint"
}

pub fn jlong() -> &'static str {
    "jni::sys::jlong"
}

pub fn jobject() -> &'static str {
    "jni::objects::JObject"
}

pub fn jstring() -> &'static str {
    "jni::objects::JString"
}