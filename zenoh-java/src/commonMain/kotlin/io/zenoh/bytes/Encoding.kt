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

import io.zenoh.jni.bytes.*

/**
 * Default encoding values used by Zenoh.
 *
 * An encoding has a similar role to Content-type in HTTP: it indicates, when present, how data should be interpreted by the application.
 *
 * Please note the Zenoh protocol does not impose any encoding value, nor it operates on it.
 * It can be seen as some optional metadata that is carried over by Zenoh in such a way the application may perform different operations depending on the encoding value.
 *
 * A set of associated constants are provided to cover the most common encodings for user convenience.
 *
 * An encoding **is** its decomposed pair `(id, schema)` — Zenoh's own
 * representation. This class is a plain immutable value; all conversion logic
 * (the id↔name table and Zenoh's parse/render rules) lives in the shared
 * bindings tier ([EncodingCodec]), verified against the native implementation
 * by `EncodingCorrespondenceTest`. No native handle, no native calls, nothing
 * to close.
 */
class Encoding internal constructor(
    internal val id: Int,
    internal val schema: String? = null,
) {

    companion object {
        @JvmField val ZENOH_BYTES = Encoding(ENCODING_ZENOH_BYTES_ID)
        @JvmField val ZENOH_STRING = Encoding(ENCODING_ZENOH_STRING_ID)
        @JvmField val ZENOH_SERIALIZED = Encoding(ENCODING_ZENOH_SERIALIZED_ID)
        @JvmField val APPLICATION_OCTET_STREAM = Encoding(ENCODING_APPLICATION_OCTET_STREAM_ID)
        @JvmField val TEXT_PLAIN = Encoding(ENCODING_TEXT_PLAIN_ID)
        @JvmField val APPLICATION_JSON = Encoding(ENCODING_APPLICATION_JSON_ID)
        @JvmField val TEXT_JSON = Encoding(ENCODING_TEXT_JSON_ID)
        @JvmField val APPLICATION_CDR = Encoding(ENCODING_APPLICATION_CDR_ID)
        @JvmField val APPLICATION_CBOR = Encoding(ENCODING_APPLICATION_CBOR_ID)
        @JvmField val APPLICATION_YAML = Encoding(ENCODING_APPLICATION_YAML_ID)
        @JvmField val TEXT_YAML = Encoding(ENCODING_TEXT_YAML_ID)
        @JvmField val TEXT_JSON5 = Encoding(ENCODING_TEXT_JSON5_ID)
        @JvmField val APPLICATION_PYTHON_SERIALIZED_OBJECT = Encoding(ENCODING_APPLICATION_PYTHON_SERIALIZED_OBJECT_ID)
        @JvmField val APPLICATION_PROTOBUF = Encoding(ENCODING_APPLICATION_PROTOBUF_ID)
        @JvmField val APPLICATION_JAVA_SERIALIZED_OBJECT = Encoding(ENCODING_APPLICATION_JAVA_SERIALIZED_OBJECT_ID)
        @JvmField val APPLICATION_OPENMETRICS_TEXT = Encoding(ENCODING_APPLICATION_OPENMETRICS_TEXT_ID)
        @JvmField val IMAGE_PNG = Encoding(ENCODING_IMAGE_PNG_ID)
        @JvmField val IMAGE_JPEG = Encoding(ENCODING_IMAGE_JPEG_ID)
        @JvmField val IMAGE_GIF = Encoding(ENCODING_IMAGE_GIF_ID)
        @JvmField val IMAGE_BMP = Encoding(ENCODING_IMAGE_BMP_ID)
        @JvmField val IMAGE_WEBP = Encoding(ENCODING_IMAGE_WEBP_ID)
        @JvmField val APPLICATION_XML = Encoding(ENCODING_APPLICATION_XML_ID)
        @JvmField val APPLICATION_X_WWW_FORM_URLENCODED = Encoding(ENCODING_APPLICATION_X_WWW_FORM_URLENCODED_ID)
        @JvmField val TEXT_HTML = Encoding(ENCODING_TEXT_HTML_ID)
        @JvmField val TEXT_XML = Encoding(ENCODING_TEXT_XML_ID)
        @JvmField val TEXT_CSS = Encoding(ENCODING_TEXT_CSS_ID)
        @JvmField val TEXT_JAVASCRIPT = Encoding(ENCODING_TEXT_JAVASCRIPT_ID)
        @JvmField val TEXT_MARKDOWN = Encoding(ENCODING_TEXT_MARKDOWN_ID)
        @JvmField val TEXT_CSV = Encoding(ENCODING_TEXT_CSV_ID)
        @JvmField val APPLICATION_SQL = Encoding(ENCODING_APPLICATION_SQL_ID)
        @JvmField val APPLICATION_COAP_PAYLOAD = Encoding(ENCODING_APPLICATION_COAP_PAYLOAD_ID)
        @JvmField val APPLICATION_JSON_PATCH_JSON = Encoding(ENCODING_APPLICATION_JSON_PATCH_JSON_ID)
        @JvmField val APPLICATION_JSON_SEQ = Encoding(ENCODING_APPLICATION_JSON_SEQ_ID)
        @JvmField val APPLICATION_JSONPATH = Encoding(ENCODING_APPLICATION_JSONPATH_ID)
        @JvmField val APPLICATION_JWT = Encoding(ENCODING_APPLICATION_JWT_ID)
        @JvmField val APPLICATION_MP4 = Encoding(ENCODING_APPLICATION_MP4_ID)
        @JvmField val APPLICATION_SOAP_XML = Encoding(ENCODING_APPLICATION_SOAP_XML_ID)
        @JvmField val APPLICATION_YANG = Encoding(ENCODING_APPLICATION_YANG_ID)
        @JvmField val AUDIO_AAC = Encoding(ENCODING_AUDIO_AAC_ID)
        @JvmField val AUDIO_FLAC = Encoding(ENCODING_AUDIO_FLAC_ID)
        @JvmField val AUDIO_MP4 = Encoding(ENCODING_AUDIO_MP4_ID)
        @JvmField val AUDIO_OGG = Encoding(ENCODING_AUDIO_OGG_ID)
        @JvmField val AUDIO_VORBIS = Encoding(ENCODING_AUDIO_VORBIS_ID)
        @JvmField val VIDEO_H261 = Encoding(ENCODING_VIDEO_H261_ID)
        @JvmField val VIDEO_H263 = Encoding(ENCODING_VIDEO_H263_ID)
        @JvmField val VIDEO_H264 = Encoding(ENCODING_VIDEO_H264_ID)
        @JvmField val VIDEO_H265 = Encoding(ENCODING_VIDEO_H265_ID)
        @JvmField val VIDEO_H266 = Encoding(ENCODING_VIDEO_H266_ID)
        @JvmField val VIDEO_MP4 = Encoding(ENCODING_VIDEO_MP4_ID)
        @JvmField val VIDEO_OGG = Encoding(ENCODING_VIDEO_OGG_ID)
        @JvmField val VIDEO_RAW = Encoding(ENCODING_VIDEO_RAW_ID)
        @JvmField val VIDEO_VP8 = Encoding(ENCODING_VIDEO_VP8_ID)
        @JvmField val VIDEO_VP9 = Encoding(ENCODING_VIDEO_VP9_ID)

        /** The default [Encoding] is [ZENOH_BYTES]. */
        @JvmStatic fun defaultEncoding() = ZENOH_BYTES

        /**
         * Parse a textual encoding (e.g. `"text/plain"`, `"text/plain;utf-8"`,
         * `"my_encoding"`). Well-known names resolve to their canonical id;
         * everything else is preserved as a custom encoding.
         */
        @JvmStatic fun from(s: String): Encoding {
            val (id, schema) = EncodingCodec.parse(s)
            return Encoding(id, schema)
        }
    }

    /**
     * Set a schema to this encoding. Zenoh does not define what a schema is and its semantics is left to the implementer.
     * E.g. a common schema for `text/plain` encoding is `utf-8`.
     */
    fun withSchema(schema: String): Encoding {
        val (nid, nschema) = EncodingCodec.withSchema(id, this.schema, schema)
        return Encoding(nid, nschema)
    }

    /** Canonical textual form. */
    override fun toString(): String = EncodingCodec.render(id, schema)

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (javaClass != other?.javaClass) return false
        other as Encoding
        return id == other.id && schema == other.schema
    }

    override fun hashCode(): Int = 31 * id + (schema?.hashCode() ?: 0)
}
