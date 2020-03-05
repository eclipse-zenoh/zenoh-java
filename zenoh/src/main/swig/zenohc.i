/*
 * Copyright (c) 2017, 2020 ADLINK Technology Inc.
 *
 * This program and the accompanying materials are made available under the
 * terms of the Eclipse Public License 2.0 which is available at
 * http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
 * which is available at https://www.apache.org/licenses/LICENSE-2.0.
 *
 * SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
 *
 * Contributors:
 *   ADLINK zenoh team, <zenoh@adlink-labs.tech>
 */

%module zenohc 


/*----- typemap for basic types -------*/
%include "stdint.i"

/*----- typemap for z_vec_t to java.util.Map<java.lang.Integer, byte[]> -------*/
%typemap(jni) z_vec_t "jobject"
%typemap(jtype) z_vec_t "java.util.Map<java.lang.Integer, byte[]>"
%typemap(jstype) z_vec_t "java.util.Map<java.lang.Integer, byte[]>"
%typemap(out) z_vec_t %{
  $result = (*jenv)->NewObject(jenv, hash_map_class, hash_map_constr);
  unsigned int len = z_vec_length(&$1);
  for(unsigned int i = 0; i < len; ++i) {
    zn_property_t *prop = (zn_property_t *)z_vec_get(&$1, i);
    jobject jinteger = (*jenv)->NewObject(jenv, integer_class, integer_constr, prop->id);
    jbyteArray jbytes = (*jenv)->NewByteArray(jenv, prop->value.length);
    (*jenv)->SetByteArrayRegion(jenv, jbytes, 0, prop->value.length, (jbyte*)prop->value.elem);
    (*jenv)->CallObjectMethod(jenv, $result, map_put_method, jinteger, jbytes);
  }
%}
%typemap(javaout) z_vec_t {
  return $jnicall;
}

/*----- typemap for java.util.Map<java.lang.Integer, byte[]> to z_vec_t* -------*/
%typemap(jni) (const z_vec_t *ps) "jobject"
%typemap(jtype) (const z_vec_t *ps) "java.util.Map.Entry<java.lang.Integer, byte[]>[]"
%typemap(jstype) (const z_vec_t *ps) "java.util.Map.Entry<java.lang.Integer, byte[]>[]"
%typemap(javain) (const z_vec_t *ps) "$javainput"
%typemap(in) (const z_vec_t *ps) %{
  z_vec_t vec; 
  if($input != NULL) {
    jsize len = (*jenv)->GetArrayLength(jenv, $input);
    assert_no_exception;
    vec = z_vec_make(len);
    for (int i = 0; i < len; ++i) {
      jobject entry = (jobject) (*jenv)->GetObjectArrayElement(jenv, $input, i);

		  jobject keyobj = (jobject) (*jenv)->CallObjectMethod(jenv, entry, entry_getKey_method);
		  jobject valobj = (jobject) (*jenv)->CallObjectMethod(jenv, entry, entry_getValue_method);

      int key = (*jenv)->CallIntMethod(jenv, keyobj, integer_intValue_method);

      z_uint8_array_t val = {
        (*jenv)->GetArrayLength(jenv, valobj), 
        (uint8_t *) (*jenv)->GetByteArrayElements(jenv, valobj, NULL)
      };
      
      z_vec_append(&vec, zn_property_make(key, val));
	  }
    $1 = &vec;
  }
%}


/*----- typemap for payload+length IN argument to ByteBuffer -------*/
%typemap(jni) (const unsigned char *payload, size_t len) "jobject"
%typemap(jtype) (const unsigned char *payload, size_t len) "java.nio.ByteBuffer"
%typemap(jstype) (const unsigned char *payload, size_t len) "java.nio.ByteBuffer"
%typemap(javain, pre="  assert $javainput.isDirect() : \"Buffer must be allocated direct.\";") (const unsigned char *payload, size_t len) "$javainput"
// %typemap(javaout) (const unsigned char *payload, size_t len) {
//   return $jnicall;
// }
%typemap(in) (const unsigned char *payload, size_t len) %{
  jbuffer_to_native(jenv, $input, $1, $2);
%}
%typemap(freearg) (const unsigned char *payload, size_t len) %{
  release_intermediate_byte_array(jenv, $input, $1, $2);
%}
// %typemap(memberin) (const unsigned char *payload, size_t len) %{
//   if ($input) {
//     $1 = $input;
//   } else {
//     $1 = 0;
//   }
// %}

/*----- typemap for zn_on_disconnect_t : erase it in Java and pass NULL to C -------*/
%typemap(in, numinputs=0) zn_on_disconnect_t on_disconnect %{
  $1 = NULL;
%}


/*----- typemap for zn_data_handler_t + arg in zn_declare_subscriber -------*/
%typemap(jni) zn_data_handler_t data_handler "jobject";
%typemap(jtype) zn_data_handler_t data_handler "org.eclipse.zenoh.net.DataHandler";
%typemap(jstype) zn_data_handler_t data_handler "org.eclipse.zenoh.net.DataHandler";
%typemap(javain) zn_data_handler_t data_handler "$javainput";
%typemap(in,numinputs=1) (zn_data_handler_t data_handler, void *arg) %{
  // Store DataHandler object in a handler_arg
  // that will be passed to jni_handledata() at each notification
  handler_arg *jarg = malloc(sizeof(handler_arg));
  jarg->handler_object = (*jenv)->NewGlobalRef(jenv, $input);
  jarg->context = NULL;
  (*jenv)->DeleteLocalRef(jenv, $input);

  $1 = jni_handledata;
  $2 = jarg;
%};

/*----- typemap for zn_data_handler_t + zn_query_handler_t + arg in zn_declare_storage -------*/
%typemap(jni) (zn_data_handler_t data_handler, zn_query_handler_t query_handler) "jobject";
%typemap(jtype) (zn_data_handler_t data_handler, zn_query_handler_t query_handler) "org.eclipse.zenoh.net.StorageHandler";
%typemap(jstype) (zn_data_handler_t data_handler, zn_query_handler_t query_handler) "org.eclipse.zenoh.net.StorageHandler";
%typemap(javain) (zn_data_handler_t data_handler, zn_query_handler_t query_handler) "$javainput";
%typemap(in,numinputs=1) (zn_data_handler_t data_handler, zn_query_handler_t query_handler, void *arg) %{
  // Store the StorageHandler object in a handler_arg
  // that will be passed to each call to jni_handledata and jni_handlequery
  handler_arg *jarg = malloc(sizeof(handler_arg));
  jarg->handler_object = (*jenv)->NewGlobalRef(jenv, $input);
  jarg->context = NULL;
  (*jenv)->DeleteLocalRef(jenv, $input);

  $1 = jni_handledata;
  $2 = jni_handlequery;
  $3 = jarg;
%};

/*----- typemap for zn_query_handler_t + arg in zn_declare_eval -------*/
%typemap(jni) (zn_query_handler_t query_handler) "jobject";
%typemap(jtype) (zn_query_handler_t query_handler) "org.eclipse.zenoh.net.QueryHandler";
%typemap(jstype) (zn_query_handler_t query_handler) "org.eclipse.zenoh.net.QueryHandler";
%typemap(javain) (zn_query_handler_t query_handler) "$javainput";
%typemap(in,numinputs=1) (zn_query_handler_t query_handler, void *arg) %{
  // Store the QueryHandler object in a handler_arg
  // that will be passed to each call to jni_handlequery
  handler_arg *jarg = malloc(sizeof(handler_arg));
  jarg->handler_object = (*jenv)->NewGlobalRef(jenv, $input);
  jarg->context = NULL;
  (*jenv)->DeleteLocalRef(jenv, $input);

  $1 = jni_handlequery;
  $2 = jarg;
%};

/*----- typemap for zn_reply_handler_t + arg in zn_query -------*/
%typemap(jni) zn_reply_handler_t reply_handler "jobject";
%typemap(jtype) zn_reply_handler_t reply_handler "org.eclipse.zenoh.net.ReplyHandler";
%typemap(jstype) zn_reply_handler_t reply_handler "org.eclipse.zenoh.net.ReplyHandler";
%typemap(javain) zn_reply_handler_t reply_handler "$javainput";
%typemap(in,numinputs=1) (zn_reply_handler_t reply_handler, void *arg) %{
  // Store ReplyHandler object in a handler_arg
  // that will be passed to jni_handlereply() at each notification
  handler_arg *jarg = malloc(sizeof(handler_arg));
  jarg->handler_object = (*jenv)->NewGlobalRef(jenv, $input);
  jarg->context = NULL;
  (*jenv)->DeleteLocalRef(jenv, $input);

  $1 = jni_handlereply;
  $2 = jarg;
%};

/*----- typemap for Resource[] to zn_resource_p_array_t -------*/
%typemap(jni) (zn_resource_p_array_t replies) "jobjectArray"
%typemap(jtype) (zn_resource_p_array_t replies) "org.eclipse.zenoh.net.Resource[]"
%typemap(jstype) (zn_resource_p_array_t replies) "org.eclipse.zenoh.net.Resource[]"
%typemap(javain) (zn_resource_p_array_t replies) "$javainput"
%typemap(in) (zn_resource_p_array_t replies) %{
  // Convert org.eclipse.zenoh.net.Resource[] into zn_resource_p_array_t
  if ($input == NULL) {
    $1.length = 0;
    $1.elem = NULL;
  } else {
    jsize len = (*jenv)->GetArrayLength(jenv, $input);
    assert_no_exception;
    $1.length = len;
    $1.elem = (zn_resource_t**)malloc(sizeof(zn_resource_t *) * $1.length);
    for (int i = 0; i < len; ++i) {
      jobject jres = (*jenv)->GetObjectArrayElement(jenv, $input, i);
      $1.elem[i] = (zn_resource_t *)malloc(sizeof(zn_resource_t));

      // rname
      jstring jrname = (jstring) (*jenv)->CallObjectMethod(jenv, jres, resource_get_rname_method);
      $1.elem[i]->rname = (*jenv)->GetStringUTFChars(jenv, jrname, 0);
      assert_no_exception;
      $1.elem[i]->context = (void*) jrname;

      // data + length
      jobject jbuffer = (*jenv)->CallObjectMethod(jenv, jres, resource_get_data_method);
      assert_no_exception;
      jbuffer_to_native(jenv, jbuffer, $1.elem[i]->data, $1.elem[i]->length);

      // encoding and kind
      $1.elem[i]->encoding = (*jenv)->CallIntMethod(jenv, jres, resource_get_encoding_method);
      assert_no_exception;
      $1.elem[i]->kind = (*jenv)->CallIntMethod(jenv, jres, resource_get_kind_method);
      assert_no_exception;
    }
  }
%}
%typemap(freearg) (const unsigned char *payload, size_t len) %{
  release_intermediate_byte_array(jenv, $input, $1, $2);
%}


%{
#include <stdint.h>
#include "zenoh/net/private/msg.h"
#include "zenoh/net/config.h"
#include "zenoh/net/recv_loop.h"
#include "zenoh/types.h"
#include "zenoh/codec.h"
#include "zenoh/rname.h"
#include "zenoh.h"
#include <assert.h>

#if (ZENOH_DEBUG == 0)
#define assert_no_exception
#else
#define assert_no_exception \
  if ((*jenv)->ExceptionCheck(jenv)) { \
    jthrowable jex = (*jenv)->ExceptionOccurred(jenv); \
    (*jenv)->Throw(jenv, jex); \
  }
#endif


/*------ Caching of Java VM, classes, methods... ------*/
JavaVM *jvm = NULL;
jclass session_class = NULL;
jclass integer_class = NULL;
jclass hash_map_class = NULL;
jclass byte_buffer_class = NULL;
jclass timestamp_class = NULL;
jclass data_info_class = NULL;
jclass reply_value_class = NULL;
jclass replies_sender_class = NULL;
jmethodID log_exception_method = NULL;
jmethodID integer_constr = NULL;
jmethodID integer_intValue_method = NULL;
jmethodID hash_map_constr = NULL;
jmethodID map_put_method = NULL;
jmethodID entry_getKey_method = NULL;
jmethodID entry_getValue_method = NULL;
jmethodID byte_buffer_is_direct_method = NULL;
jmethodID byte_buffer_has_array_method = NULL;
jmethodID byte_buffer_array_method = NULL;
jmethodID byte_buffer_array_offset_method = NULL;
jmethodID byte_buffer_position_method = NULL;
jmethodID byte_buffer_remaining_method = NULL;
jmethodID byte_buffer_wrap_method = NULL;
jmethodID handledata_method = NULL;
jmethodID handlequery_method = NULL;
jmethodID replies_sender_constr = NULL;
jmethodID handlereply_method = NULL;
jmethodID timestamp_constr = NULL;
jmethodID data_info_constr = NULL;
jmethodID reply_value_constr = NULL;
jmethodID resource_get_rname_method = NULL;
jmethodID resource_get_data_method = NULL;
jmethodID resource_get_encoding_method = NULL;
jmethodID resource_get_kind_method = NULL;


jint JNI_OnLoad(JavaVM* vm, void* reserved) {
  (void)reserved; // for warning supression as unused
  jvm = vm;
  JNIEnv* jenv;
  if ((*vm)->GetEnv(vm, (void **) &jenv, JNI_VERSION_1_6) != JNI_OK) {
    printf("Unexpected error retrieving JNIEnv in JNI_OnLoad()\n");
    return JNI_ERR;
  }

  // Caching classes. Note that we need to convert those as a GlobalRef since they are local by default and might be GCed.
  jclass s_class = (*jenv)->FindClass(jenv, "org/eclipse/zenoh/net/Session");
  assert_no_exception;
  session_class = (jclass) (*jenv)->NewGlobalRef(jenv, s_class);
  assert_no_exception;
  jclass int_class = (*jenv)->FindClass(jenv, "java/lang/Integer");
  assert_no_exception;
  integer_class = (jclass) (*jenv)->NewGlobalRef(jenv, int_class);
  assert_no_exception;
  jclass hm_class = (*jenv)->FindClass(jenv, "java/util/HashMap");
  assert_no_exception;
  hash_map_class = (jclass) (*jenv)->NewGlobalRef(jenv, hm_class);
  assert_no_exception;
  jclass bb_class = (*jenv)->FindClass(jenv, "java/nio/ByteBuffer");
  assert_no_exception;
  byte_buffer_class = (jclass) (*jenv)->NewGlobalRef(jenv, bb_class);
  assert_no_exception;
  jclass ts_class = (*jenv)->FindClass(jenv, "org/eclipse/zenoh/core/Timestamp");
  assert_no_exception;
  timestamp_class = (jclass) (*jenv)->NewGlobalRef(jenv, ts_class);
  assert_no_exception;
  jclass di_class = (*jenv)->FindClass(jenv, "org/eclipse/zenoh/net/DataInfo");
  assert_no_exception;
  data_info_class = (jclass) (*jenv)->NewGlobalRef(jenv, di_class);
  assert_no_exception;
  jclass rv_class = (*jenv)->FindClass(jenv, "org/eclipse/zenoh/net/ReplyValue");
  assert_no_exception;
  reply_value_class = (jclass) (*jenv)->NewGlobalRef(jenv, rv_class);
  assert_no_exception;
  jclass rs_class = (*jenv)->FindClass(jenv, "org/eclipse/zenoh/net/RepliesSender");
  assert_no_exception;
  replies_sender_class = (jclass) (*jenv)->NewGlobalRef(jenv, rs_class);
  assert_no_exception;

  // Non-cached classes that are used below to get methods IDs
  jclass map_class = (*jenv)->FindClass(jenv, "java/util/Map");
  assert_no_exception;
  jclass map_entry_class = (*jenv)->FindClass(jenv, "java/util/Map$Entry");
  assert_no_exception;
  jclass datahandler_class = (*jenv)->FindClass(jenv, "org/eclipse/zenoh/net/DataHandler");
  assert_no_exception;
  jclass queryhandler_class = (*jenv)->FindClass(jenv, "org/eclipse/zenoh/net/QueryHandler");
  assert_no_exception;
  jclass replyhandler_class = (*jenv)->FindClass(jenv, "org/eclipse/zenoh/net/ReplyHandler");
  assert_no_exception;
  jclass resource_class = (*jenv)->FindClass(jenv, "org/eclipse/zenoh/net/Resource");
  assert_no_exception;


  // Caching methods IDs.
  log_exception_method = (*jenv)->GetStaticMethodID(jenv, session_class,
    "LogException", "(Ljava/lang/Throwable;Ljava/lang/String;)V");
  assert_no_exception;
  integer_constr = (*jenv)->GetMethodID(jenv, integer_class,
    "<init>", "(I)V");
  assert_no_exception;
  integer_intValue_method = (*jenv)->GetMethodID(jenv, integer_class,
    "intValue", "()I");
  assert_no_exception;
  hash_map_constr = (*jenv)->GetMethodID(jenv, hash_map_class,
    "<init>", "()V");
  assert_no_exception;
  map_put_method = (*jenv)->GetMethodID(jenv, map_class,
    "put", "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;");
  assert_no_exception;
  entry_getKey_method = (*jenv)->GetMethodID(jenv, map_entry_class, 
    "getKey", "()Ljava/lang/Object;");
  assert_no_exception;
  entry_getValue_method = (*jenv)->GetMethodID(jenv, map_entry_class, 
    "getValue", "()Ljava/lang/Object;");
  assert_no_exception;
  byte_buffer_is_direct_method = (*jenv)->GetMethodID(jenv, byte_buffer_class,
    "isDirect", "()Z");
  assert_no_exception;
  byte_buffer_has_array_method = (*jenv)->GetMethodID(jenv, byte_buffer_class,
    "hasArray", "()Z");
  assert_no_exception;
  byte_buffer_array_method = (*jenv)->GetMethodID(jenv, byte_buffer_class,
    "array", "()[B");
  assert_no_exception;
  byte_buffer_array_offset_method = (*jenv)->GetMethodID(jenv, byte_buffer_class,
    "arrayOffset", "()I");
  assert_no_exception;
  byte_buffer_position_method = (*jenv)->GetMethodID(jenv, byte_buffer_class,
    "position", "()I");
  assert_no_exception;
  byte_buffer_remaining_method = (*jenv)->GetMethodID(jenv, byte_buffer_class,
    "remaining", "()I");
  assert_no_exception;
  byte_buffer_wrap_method = (*jenv)->GetStaticMethodID(jenv, byte_buffer_class,
    "wrap", "([B)Ljava/nio/ByteBuffer;");
  assert_no_exception;

  handledata_method = (*jenv)->GetMethodID(jenv, datahandler_class,
    "handleData", "(Ljava/lang/String;Ljava/nio/ByteBuffer;Lorg/eclipse/zenoh/net/DataInfo;)V");
  assert_no_exception;
  handlequery_method = (*jenv)->GetMethodID(jenv, queryhandler_class,
    "handleQuery", "(Ljava/lang/String;Ljava/lang/String;Lorg/eclipse/zenoh/net/RepliesSender;)V");
  assert_no_exception;
  replies_sender_constr = (*jenv)->GetMethodID(jenv, replies_sender_class,
    "<init>", "(JJ)V");
  assert_no_exception;
  handlereply_method = (*jenv)->GetMethodID(jenv, replyhandler_class,
   "handleReply", "(Lorg/eclipse/zenoh/net/ReplyValue;)V");
  assert_no_exception;
  timestamp_constr = (*jenv)->GetMethodID(jenv, timestamp_class,
    "<init>", "(J[B)V");
  data_info_constr = (*jenv)->GetMethodID(jenv, data_info_class,
    "<init>", "(JLorg/eclipse/zenoh/core/Timestamp;II)V");
  assert_no_exception;
  reply_value_constr = (*jenv)->GetMethodID(jenv, reply_value_class,
    "<init>", "(I[BJLjava/lang/String;Ljava/nio/ByteBuffer;Lorg/eclipse/zenoh/net/DataInfo;)V");
  assert_no_exception;
  resource_get_rname_method = (*jenv)->GetMethodID(jenv, resource_class,
   "getRname", "()Ljava/lang/String;");
  resource_get_data_method = (*jenv)->GetMethodID(jenv, resource_class,
   "getData", "()Ljava/nio/ByteBuffer;");
  resource_get_encoding_method = (*jenv)->GetMethodID(jenv, resource_class,
   "getEncoding", "()I");
  resource_get_kind_method = (*jenv)->GetMethodID(jenv, resource_class,
   "getKind", "()I");

  return JNI_VERSION_1_6;
}

void JNI_OnUnload(JavaVM *vm, void *reserved) {
  (void)reserved; // for warning supression as unused
  JNIEnv* env;
  if ((*vm)->GetEnv(vm, (void **) &env, JNI_VERSION_1_6) != JNI_OK) {
      printf("Unexpected error retrieving JNIEnv in JNI_OnUnload()\n");
      return;
  }
  
  // Delete global references to cached classes
  if (byte_buffer_class != NULL) {
    (*env)->DeleteGlobalRef(env, byte_buffer_class);
    byte_buffer_class = NULL;
  }
}

JNIEnv * get_jenv() {
  JNIEnv * jenv;
  int getEnvStat = (*jvm)->GetEnv(jvm, (void **)&jenv, JNI_VERSION_1_8);
  if (getEnvStat == JNI_OK) {
    // nothing to do
  } else if (getEnvStat == JNI_EDETACHED) {
    printf("JNI ERROR: the current thread is not attached to the JVM. Either attach it or use a Java thread.\n");
    assert(0);
  } else if (getEnvStat == JNI_EVERSION) {
    printf("JNI ERROR: JNI_VERSION_1_8 not supported\n");
    assert(0);
  } else {
    printf("JNI ERROR: unexpected status attaching current thread: %d\n", getEnvStat);
    assert(0);
  }

  return jenv;
}

int catch_and_log_exception(JNIEnv* jenv, const char* msg) {
  if ((*jenv)->ExceptionCheck(jenv)) {
    jthrowable jex = (*jenv)->ExceptionOccurred(jenv);
    jstring jmsg = (*jenv)->NewStringUTF(jenv, msg);
    (*jenv)->CallStaticVoidMethod(jenv, session_class, log_exception_method, jex, jmsg);
    (*jenv)->ExceptionClear(jenv);
    (*jenv)->DeleteLocalRef(jenv, jmsg);
    assert_no_exception;
    return 1;
  }
  return 0;
}


// Convert a Java ByteBuffer declared as 'jbuffer' into an 'unsigned char *data' and a 'int length'
// NOTE: call release_intermediate_byte_array(jenv) after usage of jbuffer
#define jbuffer_to_native(jenv, jbuffer, data, length) \
  if ((*jenv)->CallIntMethod(jenv, jbuffer, byte_buffer_is_direct_method)) { \
    data = (unsigned char *) (*jenv)->GetDirectBufferAddress(jenv, jbuffer); \
  } else if ((*jenv)->CallIntMethod(jenv, jbuffer, byte_buffer_has_array_method)) { \
    jarray array = (jbyteArray) (*jenv)->CallObjectMethod(jenv, jbuffer, byte_buffer_array_method); \
    int offset = (int) (*jenv)->CallIntMethod(jenv, jbuffer, byte_buffer_array_offset_method); \
    int position = (int) (*jenv)->CallIntMethod(jenv, jbuffer, byte_buffer_position_method); \
    jboolean is_copy; \
    data = (unsigned char *) (*jenv)->GetByteArrayElements(jenv, array, &is_copy); \
    data = &data[offset+position]; \
  } else { \
    SWIG_JavaThrowException(jenv, SWIG_JavaRuntimeException, "The ByteBuffer is neither a direct buffer, neither a wrap of an array - it's not supported"); \
  } \
  length = (int) (*jenv)->CallIntMethod(jenv, jbuffer, byte_buffer_remaining_method);

// Release an intermediate byte[] that may have been created by release_intermediate_byte_array
#define release_intermediate_byte_array(jenv, jbuffer, data, length) \
  if ((*jenv)->CallIntMethod(jenv, jbuffer, byte_buffer_has_array_method)) { \
    jarray array = (jbyteArray) (*jenv)->CallObjectMethod(jenv, jbuffer, byte_buffer_array_method); \
    int offset = (int) (*jenv)->CallIntMethod(jenv, jbuffer, byte_buffer_array_offset_method); \
    int position = (int) (*jenv)->CallIntMethod(jenv, jbuffer, byte_buffer_position_method); \
    (*jenv)->ReleaseByteArrayElements(jenv, array, (jbyte*) &data[-offset-position], JNI_ABORT); \
  }

// Convert an 'unsigned char *data' and a 'int length' into a Java ByteBuffer declared as 'jbuffer'
#define native_to_jbuffer(jenv, data, length, jbuffer) \
  jbyteArray jbuffer_array = (*jenv)->NewByteArray(jenv, length); \
  assert_no_exception; \
  (*jenv)->SetByteArrayRegion(jenv, jbuffer_array, 0, length, (const jbyte*) data); \
  assert_no_exception; \
  jbuffer = (*jenv)->CallStaticObjectMethod(jenv, byte_buffer_class, byte_buffer_wrap_method, jbuffer_array); \
  assert_no_exception; \
  (*jenv)->DeleteLocalRef(jenv, jbuffer_array); \
  assert_no_exception;

// delete a Java ByteBuffer created by native_to_jbuffer
#define delete_jbuffer(jenv, jbuffer) \
  (*jenv)->DeleteLocalRef(jenv, jbuffer); \
  assert_no_exception;



typedef struct {
  jobject handler_object;
  void *context;
} handler_arg;


void jni_handledata(const zn_resource_key_t *rkey, const unsigned char *data, size_t length, const zn_data_info_t *info, void *arg) {
  handler_arg *jarg = arg;
  JNIEnv *jenv = get_jenv();

  jstring jrname = NULL;
  if (rkey->kind == ZN_STR_RES_KEY) {
    jrname = (*jenv)->NewStringUTF(jenv, rkey->key.rname);
  } else {
    printf("INTERNAL ERROR: jni_handledata received a non-string zn_resource_key_t with kind=%d", rkey->kind);
    return;
  }

  jobject jbuffer;
  native_to_jbuffer(jenv, data, length, jbuffer);

  jbyteArray jclockid = (*jenv)->NewByteArray(jenv, 16);
  assert_no_exception;
  (*jenv)->SetByteArrayRegion(jenv, jclockid, 0, 16, (const jbyte*) info->tstamp.clock_id);
  assert_no_exception;
  jobject jtstamp = (*jenv)->NewObject(jenv, timestamp_class, timestamp_constr, info->tstamp.time, jclockid);
  assert_no_exception;

  jobject jinfo = (*jenv)->NewObject(jenv, data_info_class, data_info_constr, info->flags, jtstamp, info->encoding, info->kind);
  assert_no_exception;

  // Call DataHandler.handleData()
  (*jenv)->CallVoidMethod(jenv, jarg->handler_object, handledata_method, jrname, jbuffer, jinfo);
  catch_and_log_exception(jenv, "Exception caught calling DataHandler.handleData()");

  (*jenv)->DeleteLocalRef(jenv, jinfo);
  assert_no_exception;
  (*jenv)->DeleteLocalRef(jenv, jtstamp);
  assert_no_exception;
  (*jenv)->DeleteLocalRef(jenv, jclockid);
  assert_no_exception;
  delete_jbuffer(jenv, jbuffer);
  (*jenv)->DeleteLocalRef(jenv, jrname);
  assert_no_exception;
}

void jni_handlequery(const char *rname, const char *predicate, zn_replies_sender_t send_replies, void *query_handle, void *arg) {
  handler_arg *jarg = arg;
  JNIEnv *jenv = get_jenv();

  if (jarg->context != NULL) {
    printf("Internal error in jni_handlequery: cannot serve query, as their is already an ongoing query (context is not NULL)\n");
    zn_resource_p_array_t replies;
    replies.length = 0;
    replies.elem = NULL;
    send_replies(query_handle, replies);
    return;
  }

  jstring jrname = (*jenv)->NewStringUTF(jenv, rname);
  jstring jpredicate = (*jenv)->NewStringUTF(jenv, predicate);

  // Create RepliesSender object
  jlong send_replies_ptr = (jlong)send_replies;
  jlong query_handle_ptr = (jlong)query_handle;
  jobject jrepliesSender = (*jenv)->NewObject(jenv, replies_sender_class, replies_sender_constr,
    send_replies_ptr, query_handle_ptr);
  assert_no_exception;

  // Call QueryHandler.handleQuery()
  (*jenv)->CallVoidMethod(jenv, jarg->handler_object, handlequery_method, jrname, jpredicate, jrepliesSender);
  if (catch_and_log_exception(jenv, "XXXXX Exception caught calling QueryHandler.handleQuery()")) {
    zn_resource_p_array_t replies = {0, 0};
    send_replies(query_handle, replies);
  }

  (*jenv)->DeleteLocalRef(jenv, jrepliesSender);
  assert_no_exception;
  (*jenv)->DeleteLocalRef(jenv, jrname);
  assert_no_exception;
  (*jenv)->DeleteLocalRef(jenv, jpredicate);
  assert_no_exception;
}

void jni_handlereply(const zn_reply_value_t *reply, void *arg) {
  handler_arg *jarg = arg;
  JNIEnv *jenv = get_jenv();
  jbyteArray jsrcid = 0;
  jstring jrname = 0;
  jbyteArray jclockid = 0;
  jobject jtstamp = 0;
  jobject jinfo = 0;
  jobject jbuffer = 0;

  if (reply->kind != ZN_REPLY_FINAL) {
    jsrcid = (*jenv)->NewByteArray(jenv, reply->srcid_length);
    assert_no_exception;
    (*jenv)->SetByteArrayRegion(jenv, jsrcid, 0, reply->srcid_length, (const jbyte*) reply->srcid);
    assert_no_exception;

    if (reply->kind == ZN_STORAGE_DATA || reply->kind == ZN_EVAL_DATA) {
      jrname = (*jenv)->NewStringUTF(jenv, reply->rname);

      jclockid = (*jenv)->NewByteArray(jenv, 16);
      assert_no_exception;
      (*jenv)->SetByteArrayRegion(jenv, jclockid, 0, 16, (const jbyte*) reply->info.tstamp.clock_id);
      assert_no_exception;
      jtstamp = (*jenv)->NewObject(jenv, timestamp_class, timestamp_constr,
        reply->info.tstamp.time, jclockid);
      assert_no_exception;

      jinfo = (*jenv)->NewObject(jenv, data_info_class, data_info_constr,
        reply->info.flags, jtstamp, reply->info.encoding, reply->info.kind);
      assert_no_exception;

      native_to_jbuffer(jenv, reply->data, reply->data_length, jbuffer);
    }
  }

  jobject jreply = (*jenv)->NewObject(jenv, reply_value_class, reply_value_constr,
    reply->kind, jsrcid, reply->rsn, jrname, jbuffer, jinfo);

  // Call ReplyHandler.handleReply()
  (*jenv)->CallVoidMethod(jenv, jarg->handler_object, handlereply_method, jreply);
  catch_and_log_exception(jenv, "Exception caught calling ReplyHandler.handleReply()");

  (*jenv)->DeleteLocalRef(jenv, jreply);
  assert_no_exception;
  (*jenv)->DeleteLocalRef(jenv, jrname);
  assert_no_exception;
  (*jenv)->DeleteLocalRef(jenv, jinfo);
  assert_no_exception;
  (*jenv)->DeleteLocalRef(jenv, jtstamp);
  assert_no_exception;
  (*jenv)->DeleteLocalRef(jenv, jclockid);
  assert_no_exception;
  if (reply->kind == ZN_STORAGE_DATA || reply->kind == ZN_EVAL_DATA) {
    delete_jbuffer(jenv, jbuffer);
  }
  (*jenv)->DeleteLocalRef(jenv, jsrcid);
  assert_no_exception;
}

void call_replies_sender(jlong send_replies_ptr, jlong query_handle_ptr, zn_resource_p_array_t replies) {
  zn_replies_sender_t send_replies = (zn_replies_sender_t)send_replies_ptr;
  void* query_handle = (void*)query_handle_ptr;
  send_replies(query_handle, replies);
}

%}

void call_replies_sender(jlong send_replies_ptr, jlong query_handle_ptr, zn_resource_p_array_t replies);

#include <stdint.h>

//
// Directly include headers definint constants
// (others are partially copied below)
//
%javaconst(1);
%include "zenoh/net/result.h"
%include "zenoh/result.h"


//
// Copied from zenoh/types.h
//
typedef size_t z_vle_t;

//
// Copied from zenoh/net/property.h
//
typedef struct {
    z_vle_t origin;
    z_vle_t period;
    z_vle_t duration;
} zn_temporal_property_t;

//
// Copied from zenoh/net/types.h
//
typedef struct {  
  uint8_t kind;
  zn_temporal_property_t tprop;
} zn_sub_mode_t;

typedef void (*zn_reply_handler_t)(const zn_reply_value_t *reply, void *arg);

typedef void (*zn_data_handler_t)(const zn_resource_key_t *rkey, const unsigned char *data, size_t length, const zn_data_info_t *info, void *arg);

typedef void (*zn_replies_sender_t)(void* query_handle, zn_resource_p_array_t replies);
typedef void (*zn_query_handler_t)(const char *rname, const char *predicate, zn_replies_sender_t send_replies, void *query_handle, void *arg);

typedef struct {
  zn_session_t *z;
  z_vle_t rid;
  z_vle_t id;
} zn_sub_t;

typedef struct {
  zn_session_t *z;
  z_vle_t rid;
  z_vle_t id;
} zn_sto_t;

typedef struct {
  zn_session_t *z;
  z_vle_t rid;
  z_vle_t id;
} zn_pub_t;

typedef struct {
  zn_session_t *z;
  z_vle_t rid;
  z_vle_t id;
} zn_eva_t;

typedef struct { enum result_kind tag; union { zn_session_t * session; int error; } value;} zn_session_p_result_t; 
typedef struct { enum result_kind tag; union { zn_sub_t * sub; int error; } value;} zn_sub_p_result_t;
typedef struct { enum result_kind tag; union { zn_pub_t * pub; int error; } value;} zn_pub_p_result_t; 
typedef struct { enum result_kind tag; union { zn_sto_t * sto; int error; } value;} zn_sto_p_result_t; 
typedef struct { enum result_kind tag; union { zn_eva_t * eval; int error; } value;} zn_eval_p_result_t; 

typedef struct {
  uint8_t kind;
  uint8_t nb;
} zn_query_dest_t;

//
// Copied from zenoh/recv_loop.h
//
void* zn_recv_loop(zn_session_t* z);

int zn_running(zn_session_t* z);

int zn_start_recv_loop(zn_session_t* z);

int zn_stop_recv_loop(zn_session_t* z);


//
// Copied from zenoh/net/session.h
//
z_vec_t
zn_scout(char* iface, size_t tries, size_t period);

zn_session_p_result_t 
zn_open(char* locator, zn_on_disconnect_t on_disconnect, const z_vec_t *ps);

z_vec_t
zn_info(zn_session_t *z);

zn_sub_p_result_t 
zn_declare_subscriber(zn_session_t *z, const char* resource, const zn_sub_mode_t *sm, zn_data_handler_t data_handler, void *arg);

zn_pub_p_result_t 
zn_declare_publisher(zn_session_t *z, const char *resource);

zn_sto_p_result_t 
zn_declare_storage(zn_session_t *z, const char* resource, zn_data_handler_t data_handler, zn_query_handler_t query_handler, void *arg);

zn_eval_p_result_t 
zn_declare_eval(zn_session_t *z, const char* resource, zn_query_handler_t query_handler, void *arg);

int zn_stream_compact_data(zn_pub_t *pub, const unsigned char *payload, size_t len);
int zn_stream_data(zn_pub_t *pub, const unsigned char *payload, size_t len);
int zn_write_data(zn_session_t *z, const char* resource, const unsigned char *payload, size_t len);

int zn_stream_data_wo(zn_pub_t *pub, const unsigned char *payload, size_t len, uint8_t encoding, uint8_t kind);
int zn_write_data_wo(zn_session_t *z, const char* resource, const unsigned char *payload, size_t len, uint8_t encoding, uint8_t kind);

int zn_pull(zn_sub_t *sub);

int zn_query(zn_session_t *z, const char* resource, const char* predicate, zn_reply_handler_t reply_handler, void *arg);
int zn_query_wo(zn_session_t *z, const char* resource, const char* predicate, zn_reply_handler_t reply_handler, void *arg, zn_query_dest_t dest_storages, zn_query_dest_t dest_evals);

int zn_undeclare_subscriber(zn_sub_t *z);
int zn_undeclare_publisher(zn_pub_t *z);
int zn_undeclare_storage(zn_sto_t *z);
int zn_undeclare_eval(zn_eva_t *z);

int zn_close(zn_session_t *z);


//
// Copied from zenoh/rname.h
//
int zn_rname_intersect(char *c1, char *c2);
