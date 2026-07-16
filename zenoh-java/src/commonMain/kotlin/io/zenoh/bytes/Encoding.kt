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
 * An encoding **is** its decomposed pair `(id, schema)` — exactly Zenoh's own
 * representation; the textual form (e.g. `"text/plain;utf-8"`) is derived from
 * a fixed id↔name table. Both the table (the generated `ENCODING_*` constants,
 * regenerated from Zenoh) and the conversion rules live JVM-side, so this class
 * is a plain immutable value: no native handle, no native calls, nothing to
 * close. Its correspondence to the native implementation is verified by
 * `EncodingCorrespondenceTest`.
 *
 * For a hot publish loop with a custom schema-bearing encoding, [pinned]
 * preallocates the native form once so each call crosses only a handle —
 * see [PinnedEncoding].
 */
open class Encoding internal constructor(
    internal val id: Int,
    internal val schema: String? = null,
) {

    companion object {
        /**
         * Mirror of Zenoh's `CUSTOM_ENCODING_ID`: a custom (non-predefined)
         * encoding carries its whole textual form in the schema slot.
         */
        internal const val CUSTOM_ENCODING_ID: Int = 0xFFFF

        /**
         * The id↔name table, built from the generated constants (single source
         * of truth in Rust) plus the custom-id row Zenoh keys with the empty
         * name.
         */
        private val idToName: Map<Int, String> = mapOf(
            ENCODING_ZENOH_BYTES_ID to ENCODING_ZENOH_BYTES,
            ENCODING_ZENOH_STRING_ID to ENCODING_ZENOH_STRING,
            ENCODING_ZENOH_SERIALIZED_ID to ENCODING_ZENOH_SERIALIZED,
            ENCODING_APPLICATION_OCTET_STREAM_ID to ENCODING_APPLICATION_OCTET_STREAM,
            ENCODING_TEXT_PLAIN_ID to ENCODING_TEXT_PLAIN,
            ENCODING_APPLICATION_JSON_ID to ENCODING_APPLICATION_JSON,
            ENCODING_TEXT_JSON_ID to ENCODING_TEXT_JSON,
            ENCODING_APPLICATION_CDR_ID to ENCODING_APPLICATION_CDR,
            ENCODING_APPLICATION_CBOR_ID to ENCODING_APPLICATION_CBOR,
            ENCODING_APPLICATION_YAML_ID to ENCODING_APPLICATION_YAML,
            ENCODING_TEXT_YAML_ID to ENCODING_TEXT_YAML,
            ENCODING_TEXT_JSON5_ID to ENCODING_TEXT_JSON5,
            ENCODING_APPLICATION_PYTHON_SERIALIZED_OBJECT_ID to ENCODING_APPLICATION_PYTHON_SERIALIZED_OBJECT,
            ENCODING_APPLICATION_PROTOBUF_ID to ENCODING_APPLICATION_PROTOBUF,
            ENCODING_APPLICATION_JAVA_SERIALIZED_OBJECT_ID to ENCODING_APPLICATION_JAVA_SERIALIZED_OBJECT,
            ENCODING_APPLICATION_OPENMETRICS_TEXT_ID to ENCODING_APPLICATION_OPENMETRICS_TEXT,
            ENCODING_IMAGE_PNG_ID to ENCODING_IMAGE_PNG,
            ENCODING_IMAGE_JPEG_ID to ENCODING_IMAGE_JPEG,
            ENCODING_IMAGE_GIF_ID to ENCODING_IMAGE_GIF,
            ENCODING_IMAGE_BMP_ID to ENCODING_IMAGE_BMP,
            ENCODING_IMAGE_WEBP_ID to ENCODING_IMAGE_WEBP,
            ENCODING_APPLICATION_XML_ID to ENCODING_APPLICATION_XML,
            ENCODING_APPLICATION_X_WWW_FORM_URLENCODED_ID to ENCODING_APPLICATION_X_WWW_FORM_URLENCODED,
            ENCODING_TEXT_HTML_ID to ENCODING_TEXT_HTML,
            ENCODING_TEXT_XML_ID to ENCODING_TEXT_XML,
            ENCODING_TEXT_CSS_ID to ENCODING_TEXT_CSS,
            ENCODING_TEXT_JAVASCRIPT_ID to ENCODING_TEXT_JAVASCRIPT,
            ENCODING_TEXT_MARKDOWN_ID to ENCODING_TEXT_MARKDOWN,
            ENCODING_TEXT_CSV_ID to ENCODING_TEXT_CSV,
            ENCODING_APPLICATION_SQL_ID to ENCODING_APPLICATION_SQL,
            ENCODING_APPLICATION_COAP_PAYLOAD_ID to ENCODING_APPLICATION_COAP_PAYLOAD,
            ENCODING_APPLICATION_JSON_PATCH_JSON_ID to ENCODING_APPLICATION_JSON_PATCH_JSON,
            ENCODING_APPLICATION_JSON_SEQ_ID to ENCODING_APPLICATION_JSON_SEQ,
            ENCODING_APPLICATION_JSONPATH_ID to ENCODING_APPLICATION_JSONPATH,
            ENCODING_APPLICATION_JWT_ID to ENCODING_APPLICATION_JWT,
            ENCODING_APPLICATION_MP4_ID to ENCODING_APPLICATION_MP4,
            ENCODING_APPLICATION_SOAP_XML_ID to ENCODING_APPLICATION_SOAP_XML,
            ENCODING_APPLICATION_YANG_ID to ENCODING_APPLICATION_YANG,
            ENCODING_AUDIO_AAC_ID to ENCODING_AUDIO_AAC,
            ENCODING_AUDIO_FLAC_ID to ENCODING_AUDIO_FLAC,
            ENCODING_AUDIO_MP4_ID to ENCODING_AUDIO_MP4,
            ENCODING_AUDIO_OGG_ID to ENCODING_AUDIO_OGG,
            ENCODING_AUDIO_VORBIS_ID to ENCODING_AUDIO_VORBIS,
            ENCODING_VIDEO_H261_ID to ENCODING_VIDEO_H261,
            ENCODING_VIDEO_H263_ID to ENCODING_VIDEO_H263,
            ENCODING_VIDEO_H264_ID to ENCODING_VIDEO_H264,
            ENCODING_VIDEO_H265_ID to ENCODING_VIDEO_H265,
            ENCODING_VIDEO_H266_ID to ENCODING_VIDEO_H266,
            ENCODING_VIDEO_MP4_ID to ENCODING_VIDEO_MP4,
            ENCODING_VIDEO_OGG_ID to ENCODING_VIDEO_OGG,
            ENCODING_VIDEO_RAW_ID to ENCODING_VIDEO_RAW,
            ENCODING_VIDEO_VP8_ID to ENCODING_VIDEO_VP8,
            ENCODING_VIDEO_VP9_ID to ENCODING_VIDEO_VP9,
            CUSTOM_ENCODING_ID to "",
        )

        // Parse-side reverse table. Zenoh's `STR_TO_ID` has NO empty-string
        // row (the `CUSTOM ↔ ""` mapping is render-side only), so a leading
        // `;` input falls through to the custom arm with the whole string as
        // schema — exclude the custom row here to match.
        private val nameToId: Map<String, Int> = idToName.entries
            .filter { (_, name) -> name.isNotEmpty() }
            .associate { (id, name) -> name to id }

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
         *
         * Implements Zenoh's parse rule: everything before the first `;` is
         * looked up as a known name — a match yields `(id, rest-if-nonempty)`,
         * a miss yields the custom id with the whole string as schema.
         */
        @JvmStatic fun from(s: String): Encoding {
            if (s.isEmpty()) return ZENOH_BYTES
            val sep = s.indexOf(';')
            val name = if (sep >= 0) s.substring(0, sep) else s
            val rest = if (sep >= 0) s.substring(sep + 1) else ""
            val id = nameToId[name]
            return if (id != null) {
                Encoding(id, rest.takeIf { it.isNotEmpty() })
            } else {
                Encoding(CUSTOM_ENCODING_ID, s)
            }
        }
    }

    /**
     * Preallocate the native form of this encoding. The returned
     * [PinnedEncoding] IS this encoding (usable anywhere an [Encoding] is —
     * e.g. `PutOptions.encoding` or `PublisherOptions.encoding`), but native
     * calls pass its prebuilt handle instead of re-crossing the `(id, schema)`
     * pair — for a custom schema-bearing encoding in a hot publish loop this
     * removes the per-call string traffic. The native side only borrows the
     * handle (a cheap clone per call), so the pinned encoding is reusable
     * until [PinnedEncoding.close]s.
     */
    fun pinned(): PinnedEncoding = PinnedEncoding(id, schema)

    /**
     * Set a schema to this encoding. Zenoh does not define what a schema is and its semantics is left to the implementer.
     * E.g. a common schema for `text/plain` encoding is `utf-8`.
     *
     * For a custom encoding the schema slot holds `"name[;schema]"`; per
     * Zenoh's rule the name part is preserved and only the schema part is
     * replaced.
     */
    fun withSchema(schema: String): Encoding {
        if (id != CUSTOM_ENCODING_ID) return Encoding(id, schema)
        val name = this.schema?.substringBefore(';') ?: ""
        return Encoding(
            id,
            when {
                name.isEmpty() -> schema
                schema.isEmpty() -> name
                else -> "$name;$schema"
            },
        )
    }

    /**
     * Canonical textual form, per Zenoh's rendering rule: the known name,
     * `"name;schema"`, the bare schema for a custom encoding, or
     * `"unknown(id)"` for an unrecognized id.
     */
    override fun toString(): String {
        val name = idToName[id]
        return when {
            name == null -> if (schema == null) "unknown($id)" else "unknown($id);$schema"
            schema == null -> name
            name.isEmpty() -> schema
            else -> "$name;$schema"
        }
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        // `is`, not javaClass: a [PinnedEncoding] equals its plain counterpart
        // (pinning is a transport optimization, not part of the value).
        if (other !is Encoding) return false
        return id == other.id && schema == other.schema
    }

    override fun hashCode(): Int = 31 * id + (schema?.hashCode() ?: 0)
}

/**
 * The selector-tuple wire form of an optional encoding, matching the generated
 * externs' dual-arm expansion: `sel == -1` = absent, arm `0` = the decomposed
 * `(id, schema)` value, arm `1` = a pinned native handle (borrowed by the
 * call). Computed once per call so a concurrent [PinnedEncoding.close] cannot
 * desynchronize the tuple.
 */
internal class EncodingWire(
    val sel: Int,
    val id: Int?,
    val schema: String?,
    val handle: io.zenoh.jni.bytes.Encoding?,
)

/** See [EncodingWire]. A closed [PinnedEncoding] falls back to arm 0. */
internal fun Encoding?.forWire(): EncodingWire {
    if (this == null) return EncodingWire(-1, null, null, null)
    val h = (this as? PinnedEncoding)?.handleOrNull()
    return if (h != null) {
        EncodingWire(1, null, null, h)
    } else {
        EncodingWire(0, id, schema, null)
    }
}

/**
 * An [Encoding] with its native form preallocated (see [Encoding.pinned]).
 * Native calls pass the prebuilt handle (borrowed — cloned natively per call,
 * an `Arc` bump) instead of re-crossing the `(id, schema)` pair, removing the
 * per-call schema-string traffic in hot publish loops.
 *
 * Owns one native handle: [close] releases it deterministically (after which
 * native calls fall back to the plain `(id, schema)` crossing); a finalizer
 * backstops leaks.
 */
class PinnedEncoding internal constructor(id: Int, schema: String?) :
    Encoding(id, schema), AutoCloseable {

    internal val handle: io.zenoh.jni.bytes.Encoding =
        io.zenoh.jni.bytes.Encoding.newFromId(id, schema, io.zenoh.exceptions.throwZError0)

    /** Whether the pinned native form is still available. */
    internal fun handleOrNull(): io.zenoh.jni.bytes.Encoding? =
        handle.takeIf { !it.isClosed() }

    override fun close() {
        handle.close()
    }

    @Suppress("removal")
    protected fun finalize() {
        close()
    }
}
