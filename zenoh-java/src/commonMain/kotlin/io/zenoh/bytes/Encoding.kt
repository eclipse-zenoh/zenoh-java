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

import io.zenoh.exceptions.throwZError0
import io.zenoh.jni.bytes.ENCODING_APPLICATION_CBOR
import io.zenoh.jni.bytes.ENCODING_APPLICATION_CBOR_ID
import io.zenoh.jni.bytes.ENCODING_APPLICATION_CDR
import io.zenoh.jni.bytes.ENCODING_APPLICATION_CDR_ID
import io.zenoh.jni.bytes.ENCODING_APPLICATION_COAP_PAYLOAD
import io.zenoh.jni.bytes.ENCODING_APPLICATION_COAP_PAYLOAD_ID
import io.zenoh.jni.bytes.ENCODING_APPLICATION_JAVA_SERIALIZED_OBJECT
import io.zenoh.jni.bytes.ENCODING_APPLICATION_JAVA_SERIALIZED_OBJECT_ID
import io.zenoh.jni.bytes.ENCODING_APPLICATION_JSON
import io.zenoh.jni.bytes.ENCODING_APPLICATION_JSONPATH
import io.zenoh.jni.bytes.ENCODING_APPLICATION_JSONPATH_ID
import io.zenoh.jni.bytes.ENCODING_APPLICATION_JSON_ID
import io.zenoh.jni.bytes.ENCODING_APPLICATION_JSON_PATCH_JSON
import io.zenoh.jni.bytes.ENCODING_APPLICATION_JSON_PATCH_JSON_ID
import io.zenoh.jni.bytes.ENCODING_APPLICATION_JSON_SEQ
import io.zenoh.jni.bytes.ENCODING_APPLICATION_JSON_SEQ_ID
import io.zenoh.jni.bytes.ENCODING_APPLICATION_JWT
import io.zenoh.jni.bytes.ENCODING_APPLICATION_JWT_ID
import io.zenoh.jni.bytes.ENCODING_APPLICATION_MP4
import io.zenoh.jni.bytes.ENCODING_APPLICATION_MP4_ID
import io.zenoh.jni.bytes.ENCODING_APPLICATION_OCTET_STREAM
import io.zenoh.jni.bytes.ENCODING_APPLICATION_OCTET_STREAM_ID
import io.zenoh.jni.bytes.ENCODING_APPLICATION_OPENMETRICS_TEXT
import io.zenoh.jni.bytes.ENCODING_APPLICATION_OPENMETRICS_TEXT_ID
import io.zenoh.jni.bytes.ENCODING_APPLICATION_PROTOBUF
import io.zenoh.jni.bytes.ENCODING_APPLICATION_PROTOBUF_ID
import io.zenoh.jni.bytes.ENCODING_APPLICATION_PYTHON_SERIALIZED_OBJECT
import io.zenoh.jni.bytes.ENCODING_APPLICATION_PYTHON_SERIALIZED_OBJECT_ID
import io.zenoh.jni.bytes.ENCODING_APPLICATION_SOAP_XML
import io.zenoh.jni.bytes.ENCODING_APPLICATION_SOAP_XML_ID
import io.zenoh.jni.bytes.ENCODING_APPLICATION_SQL
import io.zenoh.jni.bytes.ENCODING_APPLICATION_SQL_ID
import io.zenoh.jni.bytes.ENCODING_APPLICATION_XML
import io.zenoh.jni.bytes.ENCODING_APPLICATION_XML_ID
import io.zenoh.jni.bytes.ENCODING_APPLICATION_X_WWW_FORM_URLENCODED
import io.zenoh.jni.bytes.ENCODING_APPLICATION_X_WWW_FORM_URLENCODED_ID
import io.zenoh.jni.bytes.ENCODING_APPLICATION_YAML
import io.zenoh.jni.bytes.ENCODING_APPLICATION_YAML_ID
import io.zenoh.jni.bytes.ENCODING_APPLICATION_YANG
import io.zenoh.jni.bytes.ENCODING_APPLICATION_YANG_ID
import io.zenoh.jni.bytes.ENCODING_AUDIO_AAC
import io.zenoh.jni.bytes.ENCODING_AUDIO_AAC_ID
import io.zenoh.jni.bytes.ENCODING_AUDIO_FLAC
import io.zenoh.jni.bytes.ENCODING_AUDIO_FLAC_ID
import io.zenoh.jni.bytes.ENCODING_AUDIO_MP4
import io.zenoh.jni.bytes.ENCODING_AUDIO_MP4_ID
import io.zenoh.jni.bytes.ENCODING_AUDIO_OGG
import io.zenoh.jni.bytes.ENCODING_AUDIO_OGG_ID
import io.zenoh.jni.bytes.ENCODING_AUDIO_VORBIS
import io.zenoh.jni.bytes.ENCODING_AUDIO_VORBIS_ID
import io.zenoh.jni.bytes.ENCODING_IMAGE_BMP
import io.zenoh.jni.bytes.ENCODING_IMAGE_BMP_ID
import io.zenoh.jni.bytes.ENCODING_IMAGE_GIF
import io.zenoh.jni.bytes.ENCODING_IMAGE_GIF_ID
import io.zenoh.jni.bytes.ENCODING_IMAGE_JPEG
import io.zenoh.jni.bytes.ENCODING_IMAGE_JPEG_ID
import io.zenoh.jni.bytes.ENCODING_IMAGE_PNG
import io.zenoh.jni.bytes.ENCODING_IMAGE_PNG_ID
import io.zenoh.jni.bytes.ENCODING_IMAGE_WEBP
import io.zenoh.jni.bytes.ENCODING_IMAGE_WEBP_ID
import io.zenoh.jni.bytes.ENCODING_TEXT_CSS
import io.zenoh.jni.bytes.ENCODING_TEXT_CSS_ID
import io.zenoh.jni.bytes.ENCODING_TEXT_CSV
import io.zenoh.jni.bytes.ENCODING_TEXT_CSV_ID
import io.zenoh.jni.bytes.ENCODING_TEXT_HTML
import io.zenoh.jni.bytes.ENCODING_TEXT_HTML_ID
import io.zenoh.jni.bytes.ENCODING_TEXT_JAVASCRIPT
import io.zenoh.jni.bytes.ENCODING_TEXT_JAVASCRIPT_ID
import io.zenoh.jni.bytes.ENCODING_TEXT_JSON
import io.zenoh.jni.bytes.ENCODING_TEXT_JSON5
import io.zenoh.jni.bytes.ENCODING_TEXT_JSON5_ID
import io.zenoh.jni.bytes.ENCODING_TEXT_JSON_ID
import io.zenoh.jni.bytes.ENCODING_TEXT_MARKDOWN
import io.zenoh.jni.bytes.ENCODING_TEXT_MARKDOWN_ID
import io.zenoh.jni.bytes.ENCODING_TEXT_PLAIN
import io.zenoh.jni.bytes.ENCODING_TEXT_PLAIN_ID
import io.zenoh.jni.bytes.ENCODING_TEXT_XML
import io.zenoh.jni.bytes.ENCODING_TEXT_XML_ID
import io.zenoh.jni.bytes.ENCODING_TEXT_YAML
import io.zenoh.jni.bytes.ENCODING_TEXT_YAML_ID
import io.zenoh.jni.bytes.ENCODING_VIDEO_H261
import io.zenoh.jni.bytes.ENCODING_VIDEO_H261_ID
import io.zenoh.jni.bytes.ENCODING_VIDEO_H263
import io.zenoh.jni.bytes.ENCODING_VIDEO_H263_ID
import io.zenoh.jni.bytes.ENCODING_VIDEO_H264
import io.zenoh.jni.bytes.ENCODING_VIDEO_H264_ID
import io.zenoh.jni.bytes.ENCODING_VIDEO_H265
import io.zenoh.jni.bytes.ENCODING_VIDEO_H265_ID
import io.zenoh.jni.bytes.ENCODING_VIDEO_H266
import io.zenoh.jni.bytes.ENCODING_VIDEO_H266_ID
import io.zenoh.jni.bytes.ENCODING_VIDEO_MP4
import io.zenoh.jni.bytes.ENCODING_VIDEO_MP4_ID
import io.zenoh.jni.bytes.ENCODING_VIDEO_OGG
import io.zenoh.jni.bytes.ENCODING_VIDEO_OGG_ID
import io.zenoh.jni.bytes.ENCODING_VIDEO_RAW
import io.zenoh.jni.bytes.ENCODING_VIDEO_RAW_ID
import io.zenoh.jni.bytes.ENCODING_VIDEO_VP8
import io.zenoh.jni.bytes.ENCODING_VIDEO_VP8_ID
import io.zenoh.jni.bytes.ENCODING_VIDEO_VP9
import io.zenoh.jni.bytes.ENCODING_VIDEO_VP9_ID
import io.zenoh.jni.bytes.ENCODING_ZENOH_BYTES
import io.zenoh.jni.bytes.ENCODING_ZENOH_BYTES_ID
import io.zenoh.jni.bytes.ENCODING_ZENOH_SERIALIZED
import io.zenoh.jni.bytes.ENCODING_ZENOH_SERIALIZED_ID
import io.zenoh.jni.bytes.ENCODING_ZENOH_STRING
import io.zenoh.jni.bytes.ENCODING_ZENOH_STRING_ID
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
 *
 * Internally the encoding is kept as its canonical textual form ([repr], e.g.
 * `"text/plain"` or `"text/plain;utf-8"`); a native `ZEncoding` handle is built
 * on demand for each native crossing (the raw `z_*` API takes the encoding by
 * reference, so the transient handle is closed by the caller after the call).
 */
class Encoding private constructor(
    private var reprLazy: String?,
    internal val id: Int?,
    private val handle: JniEncoding?,
) {

    internal constructor(repr: String) : this(repr, null, null)

    private var schemaLazy: String? = null
    private var schemaKnown: Boolean = false
    private var idCached: Int = id ?: 0
    private var idKnown: Boolean = id != null

    /**
     * Ensure the lossless decomposed form `(id, schema)` is cached. Zenoh's
     * encoding IS `(id, schema)`, so this loses nothing. A handle-backed
     * (received) Encoding already knows its [id]; its schema is read LAZILY
     * through the handle on first need. A predefined constant is born fully
     * decomposed (repr + id from the generated consts, schema known-absent) and
     * never enters this path. A repr-primary Encoding built by [from] derives
     * BOTH once from a transient handle built off [repr], then frees it —
     * caching pure JVM values and retaining no native handle. The cache is
     * reused across every native crossing, so a reused
     * encoding (the normal case — a publisher publishes one data type) never
     * re-parses its string per call.
     */
    private fun ensureDecomposed() {
        if (idKnown && schemaKnown) return
        synchronized(this) {
            if (idKnown && schemaKnown) return
            if (handle != null) {
                if (!schemaKnown) {
                    schemaLazy = handle.getSchema(throwZError0)
                    schemaKnown = true
                }
            } else {
                val h = JniEncoding.fromString(repr, throwZError0)
                try {
                    if (!idKnown) {
                        idCached = h.id(throwZError0)
                        idKnown = true
                    }
                    if (!schemaKnown) {
                        schemaLazy = h.getSchema(throwZError0)
                        schemaKnown = true
                    }
                } finally {
                    h.close()
                }
            }
        }
    }

    /**
     * Encoding id for the OUTBOUND native crossing. Cached once (see
     * [ensureDecomposed]) and reused across every put/get/reply — the wire
     * carries this cheap primitive instead of a freshly parsed string.
     */
    internal fun idForWire(): Int {
        ensureDecomposed()
        return idCached
    }

    /** Optional schema for the OUTBOUND native crossing. Cached once. */
    internal fun schemaForWire(): String? {
        ensureDecomposed()
        return schemaLazy
    }

    /**
     * Canonical display string. A handle-backed (received) Encoding
     * materializes it LAZILY on first use (toString/equals) via the native
     * accessor — received encodings are usually only forwarded or compared by
     * id, so the common path never builds the string (forward-extraction rule).
     */
    internal val repr: String
        get() = reprLazy ?: synchronized(this) {
            reprLazy
                ?: handle!!.toStr(throwZError0)
                    .also { reprLazy = it }
        }

    companion object {
        /**
         * A predefined constant: its canonical string and wire id come from
         * the generated `ENCODING_*` consts (single source of truth in Rust),
         * and a predefined encoding never carries a schema — the decomposed
         * `(id, schema)` form is fully known up front, so these constants
         * never touch native for decomposition (see [ensureDecomposed]).
         */
        private fun predefined(repr: String, id: Int): Encoding =
            Encoding(repr, id, null).apply { schemaKnown = true }

        @JvmField val ZENOH_BYTES = predefined(ENCODING_ZENOH_BYTES, ENCODING_ZENOH_BYTES_ID)
        @JvmField val ZENOH_STRING = predefined(ENCODING_ZENOH_STRING, ENCODING_ZENOH_STRING_ID)
        @JvmField val ZENOH_SERIALIZED = predefined(ENCODING_ZENOH_SERIALIZED, ENCODING_ZENOH_SERIALIZED_ID)
        @JvmField val APPLICATION_OCTET_STREAM = predefined(ENCODING_APPLICATION_OCTET_STREAM, ENCODING_APPLICATION_OCTET_STREAM_ID)
        @JvmField val TEXT_PLAIN = predefined(ENCODING_TEXT_PLAIN, ENCODING_TEXT_PLAIN_ID)
        @JvmField val APPLICATION_JSON = predefined(ENCODING_APPLICATION_JSON, ENCODING_APPLICATION_JSON_ID)
        @JvmField val TEXT_JSON = predefined(ENCODING_TEXT_JSON, ENCODING_TEXT_JSON_ID)
        @JvmField val APPLICATION_CDR = predefined(ENCODING_APPLICATION_CDR, ENCODING_APPLICATION_CDR_ID)
        @JvmField val APPLICATION_CBOR = predefined(ENCODING_APPLICATION_CBOR, ENCODING_APPLICATION_CBOR_ID)
        @JvmField val APPLICATION_YAML = predefined(ENCODING_APPLICATION_YAML, ENCODING_APPLICATION_YAML_ID)
        @JvmField val TEXT_YAML = predefined(ENCODING_TEXT_YAML, ENCODING_TEXT_YAML_ID)
        @JvmField val TEXT_JSON5 = predefined(ENCODING_TEXT_JSON5, ENCODING_TEXT_JSON5_ID)
        @JvmField val APPLICATION_PYTHON_SERIALIZED_OBJECT = predefined(ENCODING_APPLICATION_PYTHON_SERIALIZED_OBJECT, ENCODING_APPLICATION_PYTHON_SERIALIZED_OBJECT_ID)
        @JvmField val APPLICATION_PROTOBUF = predefined(ENCODING_APPLICATION_PROTOBUF, ENCODING_APPLICATION_PROTOBUF_ID)
        @JvmField val APPLICATION_JAVA_SERIALIZED_OBJECT = predefined(ENCODING_APPLICATION_JAVA_SERIALIZED_OBJECT, ENCODING_APPLICATION_JAVA_SERIALIZED_OBJECT_ID)
        @JvmField val APPLICATION_OPENMETRICS_TEXT = predefined(ENCODING_APPLICATION_OPENMETRICS_TEXT, ENCODING_APPLICATION_OPENMETRICS_TEXT_ID)
        @JvmField val IMAGE_PNG = predefined(ENCODING_IMAGE_PNG, ENCODING_IMAGE_PNG_ID)
        @JvmField val IMAGE_JPEG = predefined(ENCODING_IMAGE_JPEG, ENCODING_IMAGE_JPEG_ID)
        @JvmField val IMAGE_GIF = predefined(ENCODING_IMAGE_GIF, ENCODING_IMAGE_GIF_ID)
        @JvmField val IMAGE_BMP = predefined(ENCODING_IMAGE_BMP, ENCODING_IMAGE_BMP_ID)
        @JvmField val IMAGE_WEBP = predefined(ENCODING_IMAGE_WEBP, ENCODING_IMAGE_WEBP_ID)
        @JvmField val APPLICATION_XML = predefined(ENCODING_APPLICATION_XML, ENCODING_APPLICATION_XML_ID)
        @JvmField val APPLICATION_X_WWW_FORM_URLENCODED = predefined(ENCODING_APPLICATION_X_WWW_FORM_URLENCODED, ENCODING_APPLICATION_X_WWW_FORM_URLENCODED_ID)
        @JvmField val TEXT_HTML = predefined(ENCODING_TEXT_HTML, ENCODING_TEXT_HTML_ID)
        @JvmField val TEXT_XML = predefined(ENCODING_TEXT_XML, ENCODING_TEXT_XML_ID)
        @JvmField val TEXT_CSS = predefined(ENCODING_TEXT_CSS, ENCODING_TEXT_CSS_ID)
        @JvmField val TEXT_JAVASCRIPT = predefined(ENCODING_TEXT_JAVASCRIPT, ENCODING_TEXT_JAVASCRIPT_ID)
        @JvmField val TEXT_MARKDOWN = predefined(ENCODING_TEXT_MARKDOWN, ENCODING_TEXT_MARKDOWN_ID)
        @JvmField val TEXT_CSV = predefined(ENCODING_TEXT_CSV, ENCODING_TEXT_CSV_ID)
        @JvmField val APPLICATION_SQL = predefined(ENCODING_APPLICATION_SQL, ENCODING_APPLICATION_SQL_ID)
        @JvmField val APPLICATION_COAP_PAYLOAD = predefined(ENCODING_APPLICATION_COAP_PAYLOAD, ENCODING_APPLICATION_COAP_PAYLOAD_ID)
        @JvmField val APPLICATION_JSON_PATCH_JSON = predefined(ENCODING_APPLICATION_JSON_PATCH_JSON, ENCODING_APPLICATION_JSON_PATCH_JSON_ID)
        @JvmField val APPLICATION_JSON_SEQ = predefined(ENCODING_APPLICATION_JSON_SEQ, ENCODING_APPLICATION_JSON_SEQ_ID)
        @JvmField val APPLICATION_JSONPATH = predefined(ENCODING_APPLICATION_JSONPATH, ENCODING_APPLICATION_JSONPATH_ID)
        @JvmField val APPLICATION_JWT = predefined(ENCODING_APPLICATION_JWT, ENCODING_APPLICATION_JWT_ID)
        @JvmField val APPLICATION_MP4 = predefined(ENCODING_APPLICATION_MP4, ENCODING_APPLICATION_MP4_ID)
        @JvmField val APPLICATION_SOAP_XML = predefined(ENCODING_APPLICATION_SOAP_XML, ENCODING_APPLICATION_SOAP_XML_ID)
        @JvmField val APPLICATION_YANG = predefined(ENCODING_APPLICATION_YANG, ENCODING_APPLICATION_YANG_ID)
        @JvmField val AUDIO_AAC = predefined(ENCODING_AUDIO_AAC, ENCODING_AUDIO_AAC_ID)
        @JvmField val AUDIO_FLAC = predefined(ENCODING_AUDIO_FLAC, ENCODING_AUDIO_FLAC_ID)
        @JvmField val AUDIO_MP4 = predefined(ENCODING_AUDIO_MP4, ENCODING_AUDIO_MP4_ID)
        @JvmField val AUDIO_OGG = predefined(ENCODING_AUDIO_OGG, ENCODING_AUDIO_OGG_ID)
        @JvmField val AUDIO_VORBIS = predefined(ENCODING_AUDIO_VORBIS, ENCODING_AUDIO_VORBIS_ID)
        @JvmField val VIDEO_H261 = predefined(ENCODING_VIDEO_H261, ENCODING_VIDEO_H261_ID)
        @JvmField val VIDEO_H263 = predefined(ENCODING_VIDEO_H263, ENCODING_VIDEO_H263_ID)
        @JvmField val VIDEO_H264 = predefined(ENCODING_VIDEO_H264, ENCODING_VIDEO_H264_ID)
        @JvmField val VIDEO_H265 = predefined(ENCODING_VIDEO_H265, ENCODING_VIDEO_H265_ID)
        @JvmField val VIDEO_H266 = predefined(ENCODING_VIDEO_H266, ENCODING_VIDEO_H266_ID)
        @JvmField val VIDEO_MP4 = predefined(ENCODING_VIDEO_MP4, ENCODING_VIDEO_MP4_ID)
        @JvmField val VIDEO_OGG = predefined(ENCODING_VIDEO_OGG, ENCODING_VIDEO_OGG_ID)
        @JvmField val VIDEO_RAW = predefined(ENCODING_VIDEO_RAW, ENCODING_VIDEO_RAW_ID)
        @JvmField val VIDEO_VP8 = predefined(ENCODING_VIDEO_VP8, ENCODING_VIDEO_VP8_ID)
        @JvmField val VIDEO_VP9 = predefined(ENCODING_VIDEO_VP9, ENCODING_VIDEO_VP9_ID)

        /** The default [Encoding] is [ZENOH_BYTES]. */
        @JvmStatic fun defaultEncoding() = ZENOH_BYTES

        /**
         * Parse a textual encoding (e.g. `"text/plain"`, `"text/plain;utf-8"`,
         * `"my_encoding"`). Well-known names resolve to their canonical id;
         * everything else is preserved as a custom encoding.
         */
        @JvmStatic fun from(s: String): Encoding = Encoding(s)

        /**
         * Decodes a native `ZEncoding` handle into a value [Encoding] and frees
         * the handle. Used when an accessor / callback hands back an encoding.
         */
        internal fun fromHandle(handle: JniEncoding): Encoding {
            try {
                return Encoding(handle.toStr(throwZError0))
            } finally {
                handle.close()
            }
        }

        /** Wrap the decomposed `(handle, id)` leaves. Schema and the
         * canonical string stay lazy through the handle. */
        internal fun fromParts(encH: JniEncoding, id: Int): Encoding =
            Encoding(null, id, encH)
    }

    /**
     * Builds a fresh native `ZEncoding` handle from [repr]. The raw `z_*`
     * encoding parameters take it **by reference** (not consumed), so the
     * caller MUST close the returned handle after the native call.
     */
    internal fun toZEncoding(): JniEncoding =
        if (handle != null) {
            handle.newClone(throwZError0)
        } else {
            JniEncoding.fromString(repr, throwZError0)
        }

    /**
     * Set a schema to this encoding. Zenoh does not define what a schema is and its semantics is left to the implementer.
     * E.g. a common schema for `text/plain` encoding is `utf-8`.
     */
    fun withSchema(schema: String): Encoding {
        // `withSchema` takes the base encoding flattened to `(id, schema)`; this
        // Encoding already exposes that decomposition lazily.
        val withSchema = JniEncoding.withSchema(idForWire(), schemaForWire(), schema, throwZError0)
        try {
            return Encoding(withSchema.toStr(throwZError0))
        } finally {
            withSchema.close()
        }
    }

    override fun toString(): String = repr

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (javaClass != other?.javaClass) return false
        other as Encoding
        return repr == other.repr
    }

    override fun hashCode(): Int = repr.hashCode()
}
