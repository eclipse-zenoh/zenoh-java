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
    private val handle: io.zenoh.jni.bytes.ZEncoding?,
) {

    internal constructor(repr: String) : this(repr, null, null)

    private var schemaLazy: String? = null
    private var schemaKnown: Boolean = false

    /**
     * Optional schema. A handle-backed (received) Encoding reads it LAZILY on
     * first access (forward-extraction rule: never delivered eagerly — we
     * cannot assume any consumer reads it). A repr-primary Encoding carries
     * its schema inside [repr]; this accessor is for the received form.
     */
    internal val schema: String?
        get() {
            if (!schemaKnown) {
                synchronized(this) {
                    if (!schemaKnown) {
                        schemaLazy = handle?.let {
                            io.zenoh.jni.bytes.zEncodingSchema(it, throwZError0)
                        }
                        schemaKnown = true
                    }
                }
            }
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
                ?: io.zenoh.jni.bytes.zEncodingToString(handle!!, throwZError0)
                    .also { reprLazy = it }
        }

    companion object {
        @JvmField val ZENOH_BYTES = Encoding("zenoh/bytes")
        @JvmField val ZENOH_STRING = Encoding("zenoh/string")
        @JvmField val ZENOH_SERIALIZED = Encoding("zenoh/serialized")
        @JvmField val APPLICATION_OCTET_STREAM = Encoding("application/octet-stream")
        @JvmField val TEXT_PLAIN = Encoding("text/plain")
        @JvmField val APPLICATION_JSON = Encoding("application/json")
        @JvmField val TEXT_JSON = Encoding("text/json")
        @JvmField val APPLICATION_CDR = Encoding("application/cdr")
        @JvmField val APPLICATION_CBOR = Encoding("application/cbor")
        @JvmField val APPLICATION_YAML = Encoding("application/yaml")
        @JvmField val TEXT_YAML = Encoding("text/yaml")
        @JvmField val TEXT_JSON5 = Encoding("text/json5")
        @JvmField val APPLICATION_PYTHON_SERIALIZED_OBJECT = Encoding("application/python-serialized-object")
        @JvmField val APPLICATION_PROTOBUF = Encoding("application/protobuf")
        @JvmField val APPLICATION_JAVA_SERIALIZED_OBJECT = Encoding("application/java-serialized-object")
        @JvmField val APPLICATION_OPENMETRICS_TEXT = Encoding("application/openmetrics-text")
        @JvmField val IMAGE_PNG = Encoding("image/png")
        @JvmField val IMAGE_JPEG = Encoding("image/jpeg")
        @JvmField val IMAGE_GIF = Encoding("image/gif")
        @JvmField val IMAGE_BMP = Encoding("image/bmp")
        @JvmField val IMAGE_WEBP = Encoding("image/webp")
        @JvmField val APPLICATION_XML = Encoding("application/xml")
        @JvmField val APPLICATION_X_WWW_FORM_URLENCODED = Encoding("application/x-www-form-urlencoded")
        @JvmField val TEXT_HTML = Encoding("text/html")
        @JvmField val TEXT_XML = Encoding("text/xml")
        @JvmField val TEXT_CSS = Encoding("text/css")
        @JvmField val TEXT_JAVASCRIPT = Encoding("text/javascript")
        @JvmField val TEXT_MARKDOWN = Encoding("text/markdown")
        @JvmField val TEXT_CSV = Encoding("text/csv")
        @JvmField val APPLICATION_SQL = Encoding("application/sql")
        @JvmField val APPLICATION_COAP_PAYLOAD = Encoding("application/coap-payload")
        @JvmField val APPLICATION_JSON_PATCH_JSON = Encoding("application/json-patch+json")
        @JvmField val APPLICATION_JSON_SEQ = Encoding("application/json-seq")
        @JvmField val APPLICATION_JSONPATH = Encoding("application/jsonpath")
        @JvmField val APPLICATION_JWT = Encoding("application/jwt")
        @JvmField val APPLICATION_MP4 = Encoding("application/mp4")
        @JvmField val APPLICATION_SOAP_XML = Encoding("application/soap+xml")
        @JvmField val APPLICATION_YANG = Encoding("application/yang")
        @JvmField val AUDIO_AAC = Encoding("audio/aac")
        @JvmField val AUDIO_FLAC = Encoding("audio/flac")
        @JvmField val AUDIO_MP4 = Encoding("audio/mp4")
        @JvmField val AUDIO_OGG = Encoding("audio/ogg")
        @JvmField val AUDIO_VORBIS = Encoding("audio/vorbis")
        @JvmField val VIDEO_H261 = Encoding("video/h261")
        @JvmField val VIDEO_H263 = Encoding("video/h263")
        @JvmField val VIDEO_H264 = Encoding("video/h264")
        @JvmField val VIDEO_H265 = Encoding("video/h265")
        @JvmField val VIDEO_H266 = Encoding("video/h266")
        @JvmField val VIDEO_MP4 = Encoding("video/mp4")
        @JvmField val VIDEO_OGG = Encoding("video/ogg")
        @JvmField val VIDEO_RAW = Encoding("video/raw")
        @JvmField val VIDEO_VP8 = Encoding("video/vp8")
        @JvmField val VIDEO_VP9 = Encoding("video/vp9")

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
        internal fun fromHandle(handle: io.zenoh.jni.bytes.ZEncoding): Encoding {
            try {
                return Encoding(io.zenoh.jni.bytes.zEncodingToString(handle, throwZError0))
            } finally {
                handle.close()
            }
        }

        /** Wrap the decomposed `(handle, id)` leaves. Schema and the
         * canonical string stay lazy through the handle. */
        internal fun fromParts(encH: io.zenoh.jni.bytes.ZEncoding, id: Int): Encoding =
            Encoding(null, id, encH)
    }

    /**
     * Builds a fresh native `ZEncoding` handle from [repr]. The raw `z_*`
     * encoding parameters take it **by reference** (not consumed), so the
     * caller MUST close the returned handle after the native call.
     */
    internal fun toZEncoding(): io.zenoh.jni.bytes.ZEncoding =
        if (handle != null) {
            io.zenoh.jni.bytes.zEncodingClone(handle, throwZError0)
        } else {
            io.zenoh.jni.bytes.zEncodingFromString(repr, throwZError0)
        }

    /**
     * Set a schema to this encoding. Zenoh does not define what a schema is and its semantics is left to the implementer.
     * E.g. a common schema for `text/plain` encoding is `utf-8`.
     */
    fun withSchema(schema: String): Encoding {
        val base = toZEncoding()
        val withSchema = io.zenoh.jni.bytes.zEncodingWithSchema(base, schema, throwZError0)
        base.close()
        try {
            return Encoding(io.zenoh.jni.bytes.zEncodingToString(withSchema, throwZError0))
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
