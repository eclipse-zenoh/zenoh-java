//
// Copyright (c) 2023 ZettaScale Technology
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

use jni::{
    objects::{JByteArray, JClass, JList, JMap, JObject, JObjectArray, JString, JValue},
    sys::jobject,
    JNIEnv,
};
use zenoh::bytes::ZBytes;
use zenoh_ext::{VarInt, ZDeserializeError, ZDeserializer, ZSerializer};

use crate::{
    errors::ZResult,
    throw_exception,
    utils::{bytes_to_java_array, decode_byte_array},
    zerror,
};

enum JavaType {
    Boolean,
    String,
    ByteArray,
    Byte,
    Short,
    Int,
    Long,
    Float,
    Double,
    List(Box<JavaType>),
    Map(Box<JavaType>, Box<JavaType>),
}

fn decode_token_type(env: &mut JNIEnv, type_obj: JObject) -> ZResult<JavaType> {
    let type_name_jobject = env
        .call_method(&type_obj, "getTypeName", "()Ljava/lang/String;", &[])
        .map_err(|err| zerror!(err))?
        .l()
        .map_err(|err| zerror!(err))?;

    let qualified_name: String = env
        .get_string(&JString::from(type_name_jobject))
        .map_err(|err| zerror!(err))?
        .into();

    match qualified_name.as_str() {
        "java.lang.Boolean" => Ok(JavaType::Boolean),
        "java.lang.String" => Ok(JavaType::String),
        "byte[]" => Ok(JavaType::ByteArray),
        "java.lang.Byte" => Ok(JavaType::Byte),
        "java.lang.Short" => Ok(JavaType::Short),
        "java.lang.Integer" => Ok(JavaType::Int),
        "java.lang.Long" => Ok(JavaType::Long),
        "java.lang.Float" => Ok(JavaType::Float),
        "java.lang.Double" => Ok(JavaType::Double),
        _ => {
            let type_token_class = env
                .find_class("com/google/common/reflect/TypeToken")
                .map_err(|err| zerror!(err))?;
            let token_type = env
                .call_static_method(
                    type_token_class,
                    "of",
                    "(Ljava/lang/reflect/Type;)Lcom/google/common/reflect/TypeToken;",
                    &[JValue::Object(&type_obj)],
                )
                .map_err(|err| zerror!(err))?
                .l()
                .map_err(|err| zerror!(err))?;
            let map_class: JObject = env
                .find_class("java/util/Map")
                .map_err(|err| zerror!(err))?
                .into();
            let is_map_subtype = env
                .call_method(
                    &token_type,
                    "isSubtypeOf",
                    "(Ljava/lang/reflect/Type;)Z",
                    &[JValue::Object(&map_class)],
                )
                .map_err(|err| zerror!(err))?
                .z()
                .map_err(|err| zerror!(err))?;

            if is_map_subtype {
                let args = env
                    .call_method(
                        &type_obj,
                        "getActualTypeArguments",
                        "()[Ljava/lang/reflect/Type;",
                        &[],
                    )
                    .map_err(|err| zerror!(err))?
                    .l()
                    .map_err(|err| zerror!(err))?;
                let jobject_array = JObjectArray::from(args);
                let arg1 = env
                    .get_object_array_element(&jobject_array, 0)
                    .map_err(|err| zerror!(err))?;
                let arg2 = env
                    .get_object_array_element(&jobject_array, 1)
                    .map_err(|err| zerror!(err))?;

                return Ok(JavaType::Map(
                    Box::new(decode_token_type(env, arg1)?),
                    Box::new(decode_token_type(env, arg2)?),
                ));
            }

            let list_class: JObject = env
                .find_class("java/util/List")
                .map_err(|err| zerror!(err))?
                .into();
            let is_list_subtype = env
                .call_method(
                    &token_type,
                    "isSubtypeOf",
                    "(Ljava/lang/reflect/Type;)Z",
                    &[JValue::Object(&list_class)],
                )
                .map_err(|err| zerror!(err))?
                .z()
                .map_err(|err| zerror!(err))?;

            if is_list_subtype {
                let args = env
                    .call_method(
                        &type_obj,
                        "getActualTypeArguments",
                        "()[Ljava/lang/reflect/Type;",
                        &[],
                    )
                    .map_err(|err| zerror!(err))?
                    .l()
                    .map_err(|err| zerror!(err))?;
                let jobject_array = JObjectArray::from(args);
                let arg1 = env
                    .get_object_array_element(&jobject_array, 0)
                    .map_err(|err| zerror!(err))?;

                return Ok(JavaType::List(Box::new(decode_token_type(env, arg1)?)));
            }

            return Err(zerror!("Unsupported type: {}", qualified_name));
        }
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "C" fn Java_io_zenoh_jni_JNIZBytes_serializeViaJNI(
    mut env: JNIEnv,
    _class: JClass,
    any: JObject,
    token_type: JObject,
) -> jobject {
    || -> ZResult<jobject> {
        let mut serializer = ZSerializer::new();
        let jtype = decode_token_type(&mut env, token_type)?;
        serialize(&mut env, &mut serializer, any, &jtype)?;
        let zbytes = serializer.finish();

        let byte_array = bytes_to_java_array(&env, &zbytes).map_err(|err| zerror!(err))?;
        let zbytes_obj = env
            .new_object(
                "io/zenoh/bytes/ZBytes",
                "([B)V",
                &[JValue::Object(&JObject::from(byte_array))],
            )
            .map_err(|err| zerror!(err))?;

        Ok(zbytes_obj.as_raw())
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
    jtype: &JavaType,
) -> ZResult<()> {
    match jtype {
        JavaType::Byte => {
            let byte_value = env
                .call_method(any, "byteValue", "()B", &[])
                .map_err(|err| zerror!(err))?
                .b()
                .map_err(|err| zerror!(err))?;
            serializer.serialize(byte_value);
        }
        JavaType::Short => {
            let short_value = env
                .call_method(any, "shortValue", "()S", &[])
                .map_err(|err| zerror!(err))?
                .s()
                .map_err(|err| zerror!(err))?;
            serializer.serialize(short_value);
        }
        JavaType::Int => {
            let int_value = env
                .call_method(any, "intValue", "()I", &[])
                .map_err(|err| zerror!(err))?
                .i()
                .map_err(|err| zerror!(err))?;
            serializer.serialize(int_value);
        }
        JavaType::Long => {
            let long_value = env
                .call_method(any, "longValue", "()J", &[])
                .map_err(|err| zerror!(err))?
                .j()
                .map_err(|err| zerror!(err))?;
            serializer.serialize(long_value);
        }
        JavaType::Float => {
            let float_value = env
                .call_method(any, "floatValue", "()F", &[])
                .map_err(|err| zerror!(err))?
                .f()
                .map_err(|err| zerror!(err))?;
            serializer.serialize(float_value);
        }
        JavaType::Double => {
            let double_value = env
                .call_method(any, "doubleValue", "()D", &[])
                .map_err(|err| zerror!(err))?
                .d()
                .map_err(|err| zerror!(err))?;
            serializer.serialize(double_value);
        }
        JavaType::Boolean => {
            let boolean_value = env
                .call_method(any, "booleanValue", "()Z", &[])
                .map_err(|err| zerror!(err))?
                .z()
                .map_err(|err| zerror!(err))?;
            serializer.serialize(boolean_value);
        }
        JavaType::String => {
            let jstring = JString::from(any);
            let string_value: String = env.get_string(&jstring).map_err(|err| zerror!(err))?.into();
            serializer.serialize(string_value);
        }
        JavaType::ByteArray => {
            let jbyte_array = JByteArray::from(any);
            let bytes = decode_byte_array(env, jbyte_array).map_err(|err| zerror!(err))?;
            serializer.serialize(bytes);
        }
        JavaType::List(kotlin_type) => {
            let jlist: JList<'_, '_, '_> =
                JList::from_env(env, &any).map_err(|err| zerror!(err))?;
            let mut iterator = jlist.iter(env).map_err(|err| zerror!(err))?;
            let list_size = jlist.size(env).map_err(|err| zerror!(err))?;
            serializer.serialize(zenoh_ext::VarInt(list_size as usize));
            while let Some(value) = iterator.next(env).map_err(|err| zerror!(err))? {
                serialize(env, serializer, value, kotlin_type)?;
            }
        }
        JavaType::Map(key_type, value_type) => {
            let jmap = JMap::from_env(env, &any).map_err(|err| zerror!(err))?;

            let map_size = env
                .call_method(&jmap, "size", "()I", &[])
                .map_err(|err| zerror!(err))?
                .i()
                .map_err(|err| zerror!(err))?;

            serializer.serialize(zenoh_ext::VarInt(map_size as usize));

            let mut iterator = jmap.iter(env).map_err(|err| zerror!(err))?;
            while let Some((key, value)) = iterator.next(env).map_err(|err| zerror!(err))? {
                serialize(env, serializer, key, key_type)?;
                serialize(env, serializer, value, value_type)?;
            }
        }
    }
    Ok(())
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "C" fn Java_io_zenoh_jni_JNIZBytes_deserializeViaJNI(
    mut env: JNIEnv,
    _class: JClass,
    zbytes: JObject,
    jtype: JObject,
) -> jobject {
    || -> ZResult<jobject> {
        let payload = env
            .get_field(zbytes, "bytes", "[B")
            .map_err(|err| zerror!(err))?;
        let decoded_bytes: Vec<u8> = decode_byte_array(
            &env,
            JByteArray::from(payload.l().map_err(|err| zerror!(err))?),
        )?;
        let zbytes = ZBytes::from(decoded_bytes);
        let mut deserializer = ZDeserializer::new(&zbytes);
        let jtype = decode_token_type(&mut env, jtype)?;
        let obj = deserialize(&mut env, &mut deserializer, &jtype)?;
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
    jtype: &JavaType,
) -> ZResult<jobject> {
    match jtype {
        JavaType::Byte => {
            let byte = deserializer
                .deserialize::<i8>()
                .map_err(|err| zerror!(err))?;
            let byte_obj = env
                .new_object("java/lang/Byte", "(B)V", &[JValue::Byte(byte)])
                .map_err(|err| zerror!(err))?;
            Ok(byte_obj.as_raw())
        }
        JavaType::Short => {
            let short = deserializer
                .deserialize::<i16>()
                .map_err(|err| zerror!(err))?;
            let short_obj = env
                .new_object("java/lang/Short", "(S)V", &[JValue::Short(short)])
                .map_err(|err| zerror!(err))?;
            Ok(short_obj.as_raw())
        }
        JavaType::Int => {
            let integer = deserializer
                .deserialize::<i32>()
                .map_err(|err| zerror!(err))?;
            let integer_obj = env
                .new_object("java/lang/Integer", "(I)V", &[JValue::Int(integer)])
                .map_err(|err| zerror!(err))?;
            Ok(integer_obj.as_raw())
        }
        JavaType::Long => {
            let long = deserializer
                .deserialize::<i64>()
                .map_err(|err| zerror!(err))?;
            let long_obj = env
                .new_object("java/lang/Long", "(J)V", &[JValue::Long(long)])
                .map_err(|err| zerror!(err))?;
            Ok(long_obj.as_raw())
        }
        JavaType::Float => {
            let float = deserializer
                .deserialize::<f32>()
                .map_err(|err| zerror!(err))?;
            let float_obj = env
                .new_object("java/lang/Float", "(F)V", &[JValue::Float(float)])
                .map_err(|err| zerror!(err))?;
            Ok(float_obj.as_raw())
        }
        JavaType::Double => {
            let double = deserializer
                .deserialize::<f64>()
                .map_err(|err| zerror!(err))?;
            let double_obj = env
                .new_object("java/lang/Double", "(D)V", &[JValue::Double(double)])
                .map_err(|err| zerror!(err))?;
            Ok(double_obj.as_raw())
        }
        JavaType::Boolean => {
            let boolean_value = deserializer
                .deserialize::<bool>()
                .map_err(|err| zerror!(err))?;
            let jboolean = if boolean_value { 1u8 } else { 0u8 };
            let boolean_obj = env
                .new_object("java/lang/Boolean", "(Z)V", &[JValue::Bool(jboolean)])
                .map_err(|err| zerror!(err))?;
            Ok(boolean_obj.as_raw())
        }
        JavaType::String => {
            let deserialized_string = deserializer
                .deserialize::<String>()
                .map_err(|err| zerror!(err))?;
            let jstring = env
                .new_string(&deserialized_string)
                .map_err(|err| zerror!(err))?;
            Ok(jstring.into_raw())
        }
        JavaType::ByteArray => {
            let deserialized_bytes = deserializer
                .deserialize::<Vec<u8>>()
                .map_err(|err| zerror!(err))?;
            let jbytes = env
                .byte_array_from_slice(deserialized_bytes.as_slice())
                .map_err(|err| zerror!(err))?;
            Ok(jbytes.into_raw())
        }
        JavaType::List(kotlin_type) => {
            let list_size = deserializer
                .deserialize::<VarInt<usize>>()
                .map_err(|err| zerror!(err))?
                .0;
            let array_list = env
                .new_object("java/util/ArrayList", "()V", &[])
                .map_err(|err| zerror!(err))?;
            let jlist = JList::from_env(env, &array_list).map_err(|err| zerror!(err))?;

            for _ in 0..list_size {
                let item = deserialize(env, deserializer, kotlin_type)?;
                let item_obj = unsafe { JObject::from_raw(item) };
                jlist.add(env, &item_obj).map_err(|err| zerror!(err))?;
            }
            Ok(array_list.as_raw())
        }
        JavaType::Map(key_type, value_type) => {
            let map_size = deserializer
                .deserialize::<VarInt<usize>>()
                .map_err(|err| zerror!(err))?
                .0;
            let map = env
                .new_object("java/util/HashMap", "()V", &[])
                .map_err(|err| zerror!(err))?;
            let jmap = JMap::from_env(env, &map).map_err(|err| zerror!(err))?;

            for _ in 0..map_size {
                let key = deserialize(env, deserializer, key_type)?;
                let key_obj = unsafe { JObject::from_raw(key) };
                let value = deserialize(env, deserializer, value_type)?;
                let value_obj = unsafe { JObject::from_raw(value) };
                jmap.put(env, &key_obj, &value_obj)
                    .map_err(|err| zerror!(err))?;
            }
            Ok(map.as_raw())
        }
    }
}
