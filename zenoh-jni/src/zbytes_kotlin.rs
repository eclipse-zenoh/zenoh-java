//
// Copyright (c) 2026 ZettaScale Technology
//
// This program and the accompanying materials are made available under the
// terms of the Eclipse Public License 2.0 which is available at
// http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
// which is available at https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
//
// Contributors:
//   ZettaScale Zenoh Team, <zenoh@zettascale.tech>
//

//! JNI bridge for Kotlin-type-aware (de)serialization.
//!
//! Mirrors [zbytes.rs] but uses `kotlin.reflect.KType` instead of
//! `java.lang.reflect.Type`, enabling support for Kotlin-specific types:
//! unsigned integers (UByte, UShort, UInt, ULong) and tuples (Pair, Triple).
//!
//! Entry points are named for class `io.zenoh.jni.JNIZBytesKotlin` to avoid
//! JNI symbol conflicts with the existing `JNIZBytes` class (same method name,
//! different receiver type — JNI cannot overload on parameter type alone).

use jni::{
    objects::{JByteArray, JClass, JList, JMap, JObject, JString, JValue},
    sys::jobject,
    JNIEnv,
};
use zenoh::bytes::ZBytes;
use zenoh_ext::{VarInt, ZDeserializeError, ZDeserializer, ZSerializer};

use crate::{
    errors::ZResult,
    throw_exception,
    utils::{bytes_to_java_array, decode_byte_array},
};

enum KotlinType {
    Boolean,
    String,
    ByteArray,
    Byte,
    Short,
    Int,
    Long,
    Float,
    Double,
    UByte,
    UShort,
    UInt,
    ULong,
    List(Box<KotlinType>),
    Map(Box<KotlinType>, Box<KotlinType>),
    Pair(Box<KotlinType>, Box<KotlinType>),
    Triple(Box<KotlinType>, Box<KotlinType>, Box<KotlinType>),
}

/// Extracts the KType of the Nth type argument from a generic KType.
///
/// `ktype.getArguments()` → `List<KTypeProjection>`.
/// `projection.getType()` → `KType?`.
/// Extracts the KType at `index` from the type arguments of `ktype` and
/// immediately decodes it to [KotlinType]. Returning [KotlinType] (a pure Rust
/// type) avoids the JObject lifetime chain that arises when returning JObject.
fn decode_ktype_arg(env: &mut JNIEnv, ktype: &JObject, index: i32) -> ZResult<KotlinType> {
    let args = env
        .call_method(ktype, "getArguments", "()Ljava/util/List;", &[])
        .map_err(|err| zerror!(err))?
        .l()
        .map_err(|err| zerror!(err))?;

    let projection = env
        .call_method(&args, "get", "(I)Ljava/lang/Object;", &[JValue::Int(index)])
        .map_err(|err| zerror!(err))?
        .l()
        .map_err(|err| zerror!(err))?;

    let arg_type = env
        .call_method(&projection, "getType", "()Lkotlin/reflect/KType;", &[])
        .map_err(|err| zerror!(err))?
        .l()
        .map_err(|err| zerror!(err))?;

    if arg_type.is_null() {
        return Err(zerror!(
            "KTypeProjection.type is null (star projection not supported)"
        ));
    }
    decode_ktype(env, arg_type)
}

/// Decodes a `kotlin.reflect.KType` JVM object into a [KotlinType] enum.
fn decode_ktype(env: &mut JNIEnv, ktype: JObject) -> ZResult<KotlinType> {
    let classifier = env
        .call_method(
            &ktype,
            "getClassifier",
            "()Lkotlin/reflect/KClassifier;",
            &[],
        )
        .map_err(|err| zerror!(err))?
        .l()
        .map_err(|err| zerror!(err))?;

    if classifier.is_null() {
        return Err(zerror!(
            "KType has no classifier (star projection not supported)"
        ));
    }

    let name_obj = env
        .call_method(&classifier, "getQualifiedName", "()Ljava/lang/String;", &[])
        .map_err(|err| zerror!(err))?
        .l()
        .map_err(|err| zerror!(err))?;

    if name_obj.is_null() {
        return Err(zerror!(
            "KClass has no qualified name (anonymous/local class not supported)"
        ));
    }

    let qualified_name: std::string::String = env
        .get_string(&JString::from(name_obj))
        .map_err(|err| zerror!(err))?
        .into();

    match qualified_name.as_str() {
        "kotlin.Boolean" => Ok(KotlinType::Boolean),
        "kotlin.String" => Ok(KotlinType::String),
        "kotlin.ByteArray" => Ok(KotlinType::ByteArray),
        "kotlin.Byte" => Ok(KotlinType::Byte),
        "kotlin.Short" => Ok(KotlinType::Short),
        "kotlin.Int" => Ok(KotlinType::Int),
        "kotlin.Long" => Ok(KotlinType::Long),
        "kotlin.Float" => Ok(KotlinType::Float),
        "kotlin.Double" => Ok(KotlinType::Double),
        "kotlin.UByte" => Ok(KotlinType::UByte),
        "kotlin.UShort" => Ok(KotlinType::UShort),
        "kotlin.UInt" => Ok(KotlinType::UInt),
        "kotlin.ULong" => Ok(KotlinType::ULong),
        "kotlin.collections.List" => Ok(KotlinType::List(Box::new(decode_ktype_arg(
            env, &ktype, 0,
        )?))),
        "kotlin.collections.Map" => {
            let key = decode_ktype_arg(env, &ktype, 0)?;
            let val = decode_ktype_arg(env, &ktype, 1)?;
            Ok(KotlinType::Map(Box::new(key), Box::new(val)))
        }
        "kotlin.Pair" => {
            let first = decode_ktype_arg(env, &ktype, 0)?;
            let second = decode_ktype_arg(env, &ktype, 1)?;
            Ok(KotlinType::Pair(Box::new(first), Box::new(second)))
        }
        "kotlin.Triple" => {
            let first = decode_ktype_arg(env, &ktype, 0)?;
            let second = decode_ktype_arg(env, &ktype, 1)?;
            let third = decode_ktype_arg(env, &ktype, 2)?;
            Ok(KotlinType::Triple(
                Box::new(first),
                Box::new(second),
                Box::new(third),
            ))
        }
        _ => Err(zerror!("Unsupported Kotlin type: {}", qualified_name)),
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "C" fn Java_io_zenoh_jni_JNIZBytesKotlin_serializeViaJNI(
    mut env: JNIEnv,
    _class: JClass,
    any: JObject,
    ktype: JObject,
) -> jobject {
    || -> ZResult<jobject> {
        let kotlin_type = decode_ktype(&mut env, ktype)?;
        let mut serializer = ZSerializer::new();
        serialize(&mut env, &mut serializer, any, &kotlin_type)?;
        let zbytes = serializer.finish();
        let byte_array = bytes_to_java_array(&env, &zbytes).map_err(|err| zerror!(err))?;
        Ok(byte_array.as_raw())
    }()
    .unwrap_or_else(|err| {
        throw_exception!(env, err);
        JObject::default().as_raw()
    })
}

fn serialize(
    env: &mut JNIEnv,
    serializer: &mut ZSerializer,
    any: JObject,
    ktype: &KotlinType,
) -> ZResult<()> {
    match ktype {
        KotlinType::Boolean => {
            let v = env
                .call_method(&any, "booleanValue", "()Z", &[])
                .map_err(|err| zerror!(err))?
                .z()
                .map_err(|err| zerror!(err))?;
            serializer.serialize(v);
        }
        KotlinType::Byte => {
            let v = env
                .call_method(&any, "byteValue", "()B", &[])
                .map_err(|err| zerror!(err))?
                .b()
                .map_err(|err| zerror!(err))?;
            serializer.serialize(v);
        }
        KotlinType::Short => {
            let v = env
                .call_method(&any, "shortValue", "()S", &[])
                .map_err(|err| zerror!(err))?
                .s()
                .map_err(|err| zerror!(err))?;
            serializer.serialize(v);
        }
        KotlinType::Int => {
            let v = env
                .call_method(&any, "intValue", "()I", &[])
                .map_err(|err| zerror!(err))?
                .i()
                .map_err(|err| zerror!(err))?;
            serializer.serialize(v);
        }
        KotlinType::Long => {
            let v = env
                .call_method(&any, "longValue", "()J", &[])
                .map_err(|err| zerror!(err))?
                .j()
                .map_err(|err| zerror!(err))?;
            serializer.serialize(v);
        }
        KotlinType::Float => {
            let v = env
                .call_method(&any, "floatValue", "()F", &[])
                .map_err(|err| zerror!(err))?
                .f()
                .map_err(|err| zerror!(err))?;
            serializer.serialize(v);
        }
        KotlinType::Double => {
            let v = env
                .call_method(&any, "doubleValue", "()D", &[])
                .map_err(|err| zerror!(err))?
                .d()
                .map_err(|err| zerror!(err))?;
            serializer.serialize(v);
        }
        KotlinType::String => {
            let s: std::string::String = env
                .get_string(&JString::from(any))
                .map_err(|err| zerror!(err))?
                .into();
            serializer.serialize(s);
        }
        KotlinType::ByteArray => {
            let bytes =
                decode_byte_array(env, &JByteArray::from(any)).map_err(|err| zerror!(err))?;
            serializer.serialize(bytes);
        }
        KotlinType::UByte => {
            let v = env
                .get_field(&any, "data", "B")
                .map_err(|err| zerror!(err))?
                .b()
                .map_err(|err| zerror!(err))?;
            serializer.serialize(v as u8);
        }
        KotlinType::UShort => {
            let v = env
                .get_field(&any, "data", "S")
                .map_err(|err| zerror!(err))?
                .s()
                .map_err(|err| zerror!(err))?;
            serializer.serialize(v as u16);
        }
        KotlinType::UInt => {
            let v = env
                .get_field(&any, "data", "I")
                .map_err(|err| zerror!(err))?
                .i()
                .map_err(|err| zerror!(err))?;
            serializer.serialize(v as u32);
        }
        KotlinType::ULong => {
            let v = env
                .get_field(&any, "data", "J")
                .map_err(|err| zerror!(err))?
                .j()
                .map_err(|err| zerror!(err))?;
            serializer.serialize(v as u64);
        }
        KotlinType::List(inner) => {
            let jlist = JList::from_env(env, &any).map_err(|err| zerror!(err))?;
            let size = jlist.size(env).map_err(|err| zerror!(err))?;
            serializer.serialize(VarInt(size as usize));
            let mut iter = jlist.iter(env).map_err(|err| zerror!(err))?;
            while let Some(item) = iter.next(env).map_err(|err| zerror!(err))? {
                serialize(env, serializer, item, inner)?;
            }
        }
        KotlinType::Map(key_type, val_type) => {
            let jmap = JMap::from_env(env, &any).map_err(|err| zerror!(err))?;
            let size = env
                .call_method(&jmap, "size", "()I", &[])
                .map_err(|err| zerror!(err))?
                .i()
                .map_err(|err| zerror!(err))?;
            serializer.serialize(VarInt(size as usize));
            let mut iter = jmap.iter(env).map_err(|err| zerror!(err))?;
            while let Some((k, v)) = iter.next(env).map_err(|err| zerror!(err))? {
                serialize(env, serializer, k, key_type)?;
                serialize(env, serializer, v, val_type)?;
            }
        }
        KotlinType::Pair(first_type, second_type) => {
            let first = env
                .call_method(&any, "getFirst", "()Ljava/lang/Object;", &[])
                .map_err(|err| zerror!(err))?
                .l()
                .map_err(|err| zerror!(err))?;
            let second = env
                .call_method(&any, "getSecond", "()Ljava/lang/Object;", &[])
                .map_err(|err| zerror!(err))?
                .l()
                .map_err(|err| zerror!(err))?;
            serialize(env, serializer, first, first_type)?;
            serialize(env, serializer, second, second_type)?;
        }
        KotlinType::Triple(first_type, second_type, third_type) => {
            let first = env
                .call_method(&any, "getFirst", "()Ljava/lang/Object;", &[])
                .map_err(|err| zerror!(err))?
                .l()
                .map_err(|err| zerror!(err))?;
            let second = env
                .call_method(&any, "getSecond", "()Ljava/lang/Object;", &[])
                .map_err(|err| zerror!(err))?
                .l()
                .map_err(|err| zerror!(err))?;
            let third = env
                .call_method(&any, "getThird", "()Ljava/lang/Object;", &[])
                .map_err(|err| zerror!(err))?
                .l()
                .map_err(|err| zerror!(err))?;
            serialize(env, serializer, first, first_type)?;
            serialize(env, serializer, second, second_type)?;
            serialize(env, serializer, third, third_type)?;
        }
    }
    Ok(())
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "C" fn Java_io_zenoh_jni_JNIZBytesKotlin_deserializeViaJNI(
    mut env: JNIEnv,
    _class: JClass,
    bytes: JByteArray,
    ktype: JObject,
) -> jobject {
    || -> ZResult<jobject> {
        let raw = decode_byte_array(&mut env, &bytes)?;
        let zbytes = ZBytes::from(raw);
        let mut deserializer = ZDeserializer::new(&zbytes);
        let kotlin_type = decode_ktype(&mut env, ktype)?;
        let obj = deserialize(&mut env, &mut deserializer, &kotlin_type)?;
        if !deserializer.done() {
            return Err(zerror!(ZDeserializeError));
        }
        Ok(obj)
    }()
    .unwrap_or_else(|err| {
        throw_exception!(env, err);
        JObject::default().as_raw()
    })
}

fn deserialize(
    env: &mut JNIEnv,
    deserializer: &mut ZDeserializer,
    ktype: &KotlinType,
) -> ZResult<jobject> {
    match ktype {
        KotlinType::Boolean => {
            let v = deserializer
                .deserialize::<bool>()
                .map_err(|err| zerror!(err))?;
            let obj = env
                .new_object("java/lang/Boolean", "(Z)V", &[JValue::Bool(v as u8)])
                .map_err(|err| zerror!(err))?;
            Ok(obj.as_raw())
        }
        KotlinType::Byte => {
            let v = deserializer
                .deserialize::<i8>()
                .map_err(|err| zerror!(err))?;
            let obj = env
                .new_object("java/lang/Byte", "(B)V", &[JValue::Byte(v)])
                .map_err(|err| zerror!(err))?;
            Ok(obj.as_raw())
        }
        KotlinType::Short => {
            let v = deserializer
                .deserialize::<i16>()
                .map_err(|err| zerror!(err))?;
            let obj = env
                .new_object("java/lang/Short", "(S)V", &[JValue::Short(v)])
                .map_err(|err| zerror!(err))?;
            Ok(obj.as_raw())
        }
        KotlinType::Int => {
            let v = deserializer
                .deserialize::<i32>()
                .map_err(|err| zerror!(err))?;
            let obj = env
                .new_object("java/lang/Integer", "(I)V", &[JValue::Int(v)])
                .map_err(|err| zerror!(err))?;
            Ok(obj.as_raw())
        }
        KotlinType::Long => {
            let v = deserializer
                .deserialize::<i64>()
                .map_err(|err| zerror!(err))?;
            let obj = env
                .new_object("java/lang/Long", "(J)V", &[JValue::Long(v)])
                .map_err(|err| zerror!(err))?;
            Ok(obj.as_raw())
        }
        KotlinType::Float => {
            let v = deserializer
                .deserialize::<f32>()
                .map_err(|err| zerror!(err))?;
            let obj = env
                .new_object("java/lang/Float", "(F)V", &[JValue::Float(v)])
                .map_err(|err| zerror!(err))?;
            Ok(obj.as_raw())
        }
        KotlinType::Double => {
            let v = deserializer
                .deserialize::<f64>()
                .map_err(|err| zerror!(err))?;
            let obj = env
                .new_object("java/lang/Double", "(D)V", &[JValue::Double(v)])
                .map_err(|err| zerror!(err))?;
            Ok(obj.as_raw())
        }
        KotlinType::String => {
            let s = deserializer
                .deserialize::<std::string::String>()
                .map_err(|err| zerror!(err))?;
            let jstr = env.new_string(&s).map_err(|err| zerror!(err))?;
            Ok(jstr.into_raw())
        }
        KotlinType::ByteArray => {
            let bytes = deserializer
                .deserialize::<Vec<u8>>()
                .map_err(|err| zerror!(err))?;
            let jbytes = env
                .byte_array_from_slice(bytes.as_slice())
                .map_err(|err| zerror!(err))?;
            Ok(jbytes.into_raw())
        }
        KotlinType::UByte => {
            let v = deserializer
                .deserialize::<u8>()
                .map_err(|err| zerror!(err))?;
            let obj = env
                .new_object("kotlin/UByte", "(B)V", &[JValue::Byte(v as i8)])
                .map_err(|err| zerror!(err))?;
            Ok(obj.as_raw())
        }
        KotlinType::UShort => {
            let v = deserializer
                .deserialize::<u16>()
                .map_err(|err| zerror!(err))?;
            let obj = env
                .new_object("kotlin/UShort", "(S)V", &[JValue::Short(v as i16)])
                .map_err(|err| zerror!(err))?;
            Ok(obj.as_raw())
        }
        KotlinType::UInt => {
            let v = deserializer
                .deserialize::<u32>()
                .map_err(|err| zerror!(err))?;
            let obj = env
                .new_object("kotlin/UInt", "(I)V", &[JValue::Int(v as i32)])
                .map_err(|err| zerror!(err))?;
            Ok(obj.as_raw())
        }
        KotlinType::ULong => {
            let v = deserializer
                .deserialize::<u64>()
                .map_err(|err| zerror!(err))?;
            let obj = env
                .new_object("kotlin/ULong", "(J)V", &[JValue::Long(v as i64)])
                .map_err(|err| zerror!(err))?;
            Ok(obj.as_raw())
        }
        KotlinType::List(inner) => {
            let size = deserializer
                .deserialize::<VarInt<usize>>()
                .map_err(|err| zerror!(err))?
                .0;
            let array_list = env
                .new_object("java/util/ArrayList", "()V", &[])
                .map_err(|err| zerror!(err))?;
            let jlist = JList::from_env(env, &array_list).map_err(|err| zerror!(err))?;
            for _ in 0..size {
                let item = deserialize(env, deserializer, inner)?;
                let item_obj = unsafe { JObject::from_raw(item) };
                jlist.add(env, &item_obj).map_err(|err| zerror!(err))?;
            }
            Ok(array_list.as_raw())
        }
        KotlinType::Map(key_type, val_type) => {
            let size = deserializer
                .deserialize::<VarInt<usize>>()
                .map_err(|err| zerror!(err))?
                .0;
            let hash_map = env
                .new_object("java/util/HashMap", "()V", &[])
                .map_err(|err| zerror!(err))?;
            let jmap = JMap::from_env(env, &hash_map).map_err(|err| zerror!(err))?;
            for _ in 0..size {
                let k = deserialize(env, deserializer, key_type)?;
                let k_obj = unsafe { JObject::from_raw(k) };
                let v = deserialize(env, deserializer, val_type)?;
                let v_obj = unsafe { JObject::from_raw(v) };
                jmap.put(env, &k_obj, &v_obj).map_err(|err| zerror!(err))?;
            }
            Ok(hash_map.as_raw())
        }
        KotlinType::Pair(first_type, second_type) => {
            let first = deserialize(env, deserializer, first_type)?;
            let second = deserialize(env, deserializer, second_type)?;
            let first_obj = unsafe { JObject::from_raw(first) };
            let second_obj = unsafe { JObject::from_raw(second) };
            let pair = env
                .new_object(
                    "kotlin/Pair",
                    "(Ljava/lang/Object;Ljava/lang/Object;)V",
                    &[JValue::Object(&first_obj), JValue::Object(&second_obj)],
                )
                .map_err(|err| zerror!(err))?;
            Ok(pair.as_raw())
        }
        KotlinType::Triple(first_type, second_type, third_type) => {
            let first = deserialize(env, deserializer, first_type)?;
            let second = deserialize(env, deserializer, second_type)?;
            let third = deserialize(env, deserializer, third_type)?;
            let first_obj = unsafe { JObject::from_raw(first) };
            let second_obj = unsafe { JObject::from_raw(second) };
            let third_obj = unsafe { JObject::from_raw(third) };
            let triple = env
                .new_object(
                    "kotlin/Triple",
                    "(Ljava/lang/Object;Ljava/lang/Object;Ljava/lang/Object;)V",
                    &[
                        JValue::Object(&first_obj),
                        JValue::Object(&second_obj),
                        JValue::Object(&third_obj),
                    ],
                )
                .map_err(|err| zerror!(err))?;
            Ok(triple.as_raw())
        }
    }
}
