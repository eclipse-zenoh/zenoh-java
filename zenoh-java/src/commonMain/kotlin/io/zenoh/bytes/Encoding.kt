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

package io.zenoh.bytes

import io.zenoh.jni.bytes.Encoding as JniEncoding

/**
 * Default encoding values used by Zenoh.
 *
 * An encoding has a similar role to Content-type in HTTP: it indicates, when present, how data should be interpreted by the application.
 *
 * Please note the Zenoh protocol does not impose any encoding value, nor it operates on it.
 * It can be seen as some optional metadata that is carried over by Zenoh in such a way the application may perform different operations depending on the encoding value.
 *
 * A set of associated constants are provided to cover the most common encodings for user convenience.
 * This is particularly useful in helping Zenoh to perform additional network optimizations.
 */
class Encoding internal constructor(internal val inner: JniEncoding) {

    internal constructor(id: Int, schema: String? = null) : this(JniEncoding(id, schema))

    internal val id: Int get() = inner.id
    internal val schema: String? get() = inner.schema

    companion object {

        /** Just some bytes. Constant alias for string: `"zenoh/bytes"`. */
        @JvmField val ZENOH_BYTES = Encoding(JniEncoding.zEncodingZenohBytes())

        /** A UTF-8 string. Constant alias for string: `"zenoh/string"`. */
        @JvmField val ZENOH_STRING = Encoding(JniEncoding.zEncodingZenohString())

        /** Zenoh serialized data. Constant alias for string: `"zenoh/serialized"`. */
        @JvmField val ZENOH_SERIALIZED = Encoding(JniEncoding.zEncodingZenohSerialized())

        /** An application-specific stream of bytes. Constant alias for string: `"application/octet-stream"`. */
        @JvmField val APPLICATION_OCTET_STREAM = Encoding(JniEncoding.zEncodingApplicationOctetStream())

        /** A textual file. Constant alias for string: `"text/plain"`. */
        @JvmField val TEXT_PLAIN = Encoding(JniEncoding.zEncodingTextPlain())

        /** JSON data intended to be consumed by an application. Constant alias for string: `"application/json"`. */
        @JvmField val APPLICATION_JSON = Encoding(JniEncoding.zEncodingApplicationJson())

        /** JSON data intended to be human readable. Constant alias for string: `"text/json"`. */
        @JvmField val TEXT_JSON = Encoding(JniEncoding.zEncodingTextJson())

        /** A Common Data Representation (CDR)-encoded data. Constant alias for string: `"application/cdr"`. */
        @JvmField val APPLICATION_CDR = Encoding(JniEncoding.zEncodingApplicationCdr())

        /** A Concise Binary Object Representation (CBOR)-encoded data. Constant alias for string: `"application/cbor"`. */
        @JvmField val APPLICATION_CBOR = Encoding(JniEncoding.zEncodingApplicationCbor())

        /** YAML data intended to be consumed by an application. Constant alias for string: `"application/yaml"`. */
        @JvmField val APPLICATION_YAML = Encoding(JniEncoding.zEncodingApplicationYaml())

        /** YAML data intended to be human readable. Constant alias for string: `"text/yaml"`. */
        @JvmField val TEXT_YAML = Encoding(JniEncoding.zEncodingTextYaml())

        /** JSON5 encoded data that are human readable. Constant alias for string: `"text/json5"`. */
        @JvmField val TEXT_JSON5 = Encoding(JniEncoding.zEncodingTextJson5())

        /** A Python object serialized using [pickle](https://docs.python.org/3/library/pickle.html). Constant alias for string: `"application/python-serialized-object"`. */
        @JvmField val APPLICATION_PYTHON_SERIALIZED_OBJECT = Encoding(JniEncoding.zEncodingApplicationPythonSerializedObject())

        /** An application-specific protobuf-encoded data. Constant alias for string: `"application/protobuf"`. */
        @JvmField val APPLICATION_PROTOBUF = Encoding(JniEncoding.zEncodingApplicationProtobuf())

        /** A Java serialized object. Constant alias for string: `"application/java-serialized-object"`. */
        @JvmField val APPLICATION_JAVA_SERIALIZED_OBJECT = Encoding(JniEncoding.zEncodingApplicationJavaSerializedObject())

        /** OpenMetrics data, commonly used by [Prometheus](https://prometheus.io/). Constant alias for string: `"application/openmetrics-text"`. */
        @JvmField val APPLICATION_OPENMETRICS_TEXT = Encoding(JniEncoding.zEncodingApplicationOpenmetricsText())

        /** A Portable Network Graphics (PNG) image. Constant alias for string: `"image/png"`. */
        @JvmField val IMAGE_PNG = Encoding(JniEncoding.zEncodingImagePng())

        /** A Joint Photographic Experts Group (JPEG) image. Constant alias for string: `"image/jpeg"`. */
        @JvmField val IMAGE_JPEG = Encoding(JniEncoding.zEncodingImageJpeg())

        /** A Graphics Interchange Format (GIF) image. Constant alias for string: `"image/gif"`. */
        @JvmField val IMAGE_GIF = Encoding(JniEncoding.zEncodingImageGif())

        /** A BitMap (BMP) image. Constant alias for string: `"image/bmp"`. */
        @JvmField val IMAGE_BMP = Encoding(JniEncoding.zEncodingImageBmp())

        /** A WebP image. Constant alias for string: `"image/webp"`. */
        @JvmField val IMAGE_WEBP = Encoding(JniEncoding.zEncodingImageWebp())

        /** An XML file intended to be consumed by an application. Constant alias for string: `"application/xml"`. */
        @JvmField val APPLICATION_XML = Encoding(JniEncoding.zEncodingApplicationXml())

        /** A list of tuples, each consisting of a name and a value. Constant alias for string: `"application/x-www-form-urlencoded"`. */
        @JvmField val APPLICATION_X_WWW_FORM_URLENCODED = Encoding(JniEncoding.zEncodingApplicationXWwwFormUrlencoded())

        /** An HTML file. Constant alias for string: `"text/html"`. */
        @JvmField val TEXT_HTML = Encoding(JniEncoding.zEncodingTextHtml())

        /** An XML file that is human readable. Constant alias for string: `"text/xml"`. */
        @JvmField val TEXT_XML = Encoding(JniEncoding.zEncodingTextXml())

        /** A CSS file. Constant alias for string: `"text/css"`. */
        @JvmField val TEXT_CSS = Encoding(JniEncoding.zEncodingTextCss())

        /** A JavaScript file. Constant alias for string: `"text/javascript"`. */
        @JvmField val TEXT_JAVASCRIPT = Encoding(JniEncoding.zEncodingTextJavascript())

        /** A Markdown file. Constant alias for string: `"text/markdown"`. */
        @JvmField val TEXT_MARKDOWN = Encoding(JniEncoding.zEncodingTextMarkdown())

        /** A CSV file. Constant alias for string: `"text/csv"`. */
        @JvmField val TEXT_CSV = Encoding(JniEncoding.zEncodingTextCsv())

        /** An application-specific SQL query. Constant alias for string: `"application/sql"`. */
        @JvmField val APPLICATION_SQL = Encoding(JniEncoding.zEncodingApplicationSql())

        /** Constrained Application Protocol (CoAP) data intended for CoAP-to-HTTP and HTTP-to-CoAP proxies. Constant alias for string: `"application/coap-payload"`. */
        @JvmField val APPLICATION_COAP_PAYLOAD = Encoding(JniEncoding.zEncodingApplicationCoapPayload())

        /** Defines a JSON document structure for expressing a sequence of operations to apply to a JSON document. Constant alias for string: `"application/json-patch+json"`. */
        @JvmField val APPLICATION_JSON_PATCH_JSON = Encoding(JniEncoding.zEncodingApplicationJsonPatchJson())

        /** A JSON text sequence consists of any number of JSON texts, all encoded in UTF-8. Constant alias for string: `"application/json-seq"`. */
        @JvmField val APPLICATION_JSON_SEQ = Encoding(JniEncoding.zEncodingApplicationJsonSeq())

        /** A JSONPath defines a string syntax for selecting and extracting JSON values from within a given JSON value. Constant alias for string: `"application/jsonpath"`. */
        @JvmField val APPLICATION_JSONPATH = Encoding(JniEncoding.zEncodingApplicationJsonpath())

        /** A JSON Web Token (JWT). Constant alias for string: `"application/jwt"`. */
        @JvmField val APPLICATION_JWT = Encoding(JniEncoding.zEncodingApplicationJwt())

        /** An application-specific MPEG-4 encoded data, either audio or video. Constant alias for string: `"application/mp4"`. */
        @JvmField val APPLICATION_MP4 = Encoding(JniEncoding.zEncodingApplicationMp4())

        /** A SOAP 1.2 message serialized as XML 1.0. Constant alias for string: `"application/soap+xml"`. */
        @JvmField val APPLICATION_SOAP_XML = Encoding(JniEncoding.zEncodingApplicationSoapXml())

        /** A YANG-encoded data commonly used by the Network Configuration Protocol (NETCONF). Constant alias for string: `"application/yang"`. */
        @JvmField val APPLICATION_YANG = Encoding(JniEncoding.zEncodingApplicationYang())

        /** A MPEG-4 Advanced Audio Coding (AAC) media. Constant alias for string: `"audio/aac"`. */
        @JvmField val AUDIO_AAC = Encoding(JniEncoding.zEncodingAudioAac())

        /** A Free Lossless Audio Codec (FLAC) media. Constant alias for string: `"audio/flac"`. */
        @JvmField val AUDIO_FLAC = Encoding(JniEncoding.zEncodingAudioFlac())

        /** An audio codec defined in MPEG-1, MPEG-2, MPEG-4, or registered at the MP4 registration authority. Constant alias for string: `"audio/mp4"`. */
        @JvmField val AUDIO_MP4 = Encoding(JniEncoding.zEncodingAudioMp4())

        /** An Ogg-encapsulated audio stream. Constant alias for string: `"audio/ogg"`. */
        @JvmField val AUDIO_OGG = Encoding(JniEncoding.zEncodingAudioOgg())

        /** A Vorbis-encoded audio stream. Constant alias for string: `"audio/vorbis"`. */
        @JvmField val AUDIO_VORBIS = Encoding(JniEncoding.zEncodingAudioVorbis())

        /** A h261-encoded video stream. Constant alias for string: `"video/h261"`. */
        @JvmField val VIDEO_H261 = Encoding(JniEncoding.zEncodingVideoH261())

        /** A h263-encoded video stream. Constant alias for string: `"video/h263"`. */
        @JvmField val VIDEO_H263 = Encoding(JniEncoding.zEncodingVideoH263())

        /** A h264-encoded video stream. Constant alias for string: `"video/h264"`. */
        @JvmField val VIDEO_H264 = Encoding(JniEncoding.zEncodingVideoH264())

        /** A h265-encoded video stream. Constant alias for string: `"video/h265"`. */
        @JvmField val VIDEO_H265 = Encoding(JniEncoding.zEncodingVideoH265())

        /** A h266-encoded video stream. Constant alias for string: `"video/h266"`. */
        @JvmField val VIDEO_H266 = Encoding(JniEncoding.zEncodingVideoH266())

        /** A video codec defined in MPEG-1, MPEG-2, MPEG-4, or registered at the MP4 registration authority. Constant alias for string: `"video/mp4"`. */
        @JvmField val VIDEO_MP4 = Encoding(JniEncoding.zEncodingVideoMp4())

        /** An Ogg-encapsulated video stream. Constant alias for string: `"video/ogg"`. */
        @JvmField val VIDEO_OGG = Encoding(JniEncoding.zEncodingVideoOgg())

        /** An uncompressed, studio-quality video stream. Constant alias for string: `"video/raw"`. */
        @JvmField val VIDEO_RAW = Encoding(JniEncoding.zEncodingVideoRaw())

        /** A VP8-encoded video stream. Constant alias for string: `"video/vp8"`. */
        @JvmField val VIDEO_VP8 = Encoding(JniEncoding.zEncodingVideoVp8())

        /** A VP9-encoded video stream. Constant alias for string: `"video/vp9"`. */
        @JvmField val VIDEO_VP9 = Encoding(JniEncoding.zEncodingVideoVp9())

        /** The default [Encoding] is [ZENOH_BYTES]. */
        @JvmStatic fun defaultEncoding() = ZENOH_BYTES
    }

    /**
     * Set a schema to this encoding. Zenoh does not define what a schema is and its semantics is left to the implementer.
     * E.g. a common schema for `text/plain` encoding is `utf-8`.
     */
    fun withSchema(schema: String): Encoding =
        Encoding(JniEncoding(inner.id, schema))

    private val cachedString: String by lazy { inner.zEncodingToString() }

    override fun toString(): String = cachedString

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (javaClass != other?.javaClass) return false
        other as Encoding
        return inner == other.inner
    }

    override fun hashCode(): Int = inner.hashCode()

    internal fun toFlat(): JniEncoding = inner
}
