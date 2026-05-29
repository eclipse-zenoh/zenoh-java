use crate::ZEncoding;
use prebindgen_proc_macro::prebindgen;

/// Numeric id of the encoding (u16 widened to i32 for JVM).
#[prebindgen]
pub fn z_encoding_id(e: &ZEncoding) -> i32 {
    e.id() as i32
}

/// Optional textual schema attached to the encoding.
#[prebindgen]
pub fn z_encoding_schema(e: &ZEncoding) -> Option<String> {
    e.schema()
        .and_then(|s| std::str::from_utf8(s).ok().map(str::to_string))
}

/// Canonical display string for a [`ZEncoding`] (upstream `Display` impl).
#[prebindgen]
pub fn z_encoding_to_string(e: &ZEncoding) -> String {
    e.to_string()
}

/// Parse a textual encoding into a [`ZEncoding`] (upstream `From<String>`:
/// known names resolve to their canonical id; everything else is preserved
/// under the custom-encoding id).
#[prebindgen]
pub fn z_encoding_from_string(s: String) -> ZEncoding {
    ZEncoding::from(s)
}

// ── Predefined-constant accessors ─────────────────────────────────────────
// ZEncoding-side parallels of `encoding_*` in encoding.rs.

#[prebindgen]
pub fn z_encoding_zenoh_bytes() -> ZEncoding { ZEncoding::ZENOH_BYTES }

#[prebindgen]
pub fn z_encoding_zenoh_string() -> ZEncoding { ZEncoding::ZENOH_STRING }

#[prebindgen]
pub fn z_encoding_zenoh_serialized() -> ZEncoding { ZEncoding::ZENOH_SERIALIZED }

#[prebindgen]
pub fn z_encoding_application_octet_stream() -> ZEncoding { ZEncoding::APPLICATION_OCTET_STREAM }

#[prebindgen]
pub fn z_encoding_text_plain() -> ZEncoding { ZEncoding::TEXT_PLAIN }

#[prebindgen]
pub fn z_encoding_application_json() -> ZEncoding { ZEncoding::APPLICATION_JSON }

#[prebindgen]
pub fn z_encoding_text_json() -> ZEncoding { ZEncoding::TEXT_JSON }

#[prebindgen]
pub fn z_encoding_application_cdr() -> ZEncoding { ZEncoding::APPLICATION_CDR }

#[prebindgen]
pub fn z_encoding_application_cbor() -> ZEncoding { ZEncoding::APPLICATION_CBOR }

#[prebindgen]
pub fn z_encoding_application_yaml() -> ZEncoding { ZEncoding::APPLICATION_YAML }

#[prebindgen]
pub fn z_encoding_text_yaml() -> ZEncoding { ZEncoding::TEXT_YAML }

#[prebindgen]
pub fn z_encoding_text_json5() -> ZEncoding { ZEncoding::TEXT_JSON5 }

#[prebindgen]
pub fn z_encoding_application_python_serialized_object() -> ZEncoding { ZEncoding::APPLICATION_PYTHON_SERIALIZED_OBJECT }

#[prebindgen]
pub fn z_encoding_application_protobuf() -> ZEncoding { ZEncoding::APPLICATION_PROTOBUF }

#[prebindgen]
pub fn z_encoding_application_java_serialized_object() -> ZEncoding { ZEncoding::APPLICATION_JAVA_SERIALIZED_OBJECT }

#[prebindgen]
pub fn z_encoding_application_openmetrics_text() -> ZEncoding { ZEncoding::APPLICATION_OPENMETRICS_TEXT }

#[prebindgen]
pub fn z_encoding_image_png() -> ZEncoding { ZEncoding::IMAGE_PNG }

#[prebindgen]
pub fn z_encoding_image_jpeg() -> ZEncoding { ZEncoding::IMAGE_JPEG }

#[prebindgen]
pub fn z_encoding_image_gif() -> ZEncoding { ZEncoding::IMAGE_GIF }

#[prebindgen]
pub fn z_encoding_image_bmp() -> ZEncoding { ZEncoding::IMAGE_BMP }

#[prebindgen]
pub fn z_encoding_image_webp() -> ZEncoding { ZEncoding::IMAGE_WEBP }

#[prebindgen]
pub fn z_encoding_application_xml() -> ZEncoding { ZEncoding::APPLICATION_XML }

#[prebindgen]
pub fn z_encoding_application_x_www_form_urlencoded() -> ZEncoding { ZEncoding::APPLICATION_X_WWW_FORM_URLENCODED }

#[prebindgen]
pub fn z_encoding_text_html() -> ZEncoding { ZEncoding::TEXT_HTML }

#[prebindgen]
pub fn z_encoding_text_xml() -> ZEncoding { ZEncoding::TEXT_XML }

#[prebindgen]
pub fn z_encoding_text_css() -> ZEncoding { ZEncoding::TEXT_CSS }

#[prebindgen]
pub fn z_encoding_text_javascript() -> ZEncoding { ZEncoding::TEXT_JAVASCRIPT }

#[prebindgen]
pub fn z_encoding_text_markdown() -> ZEncoding { ZEncoding::TEXT_MARKDOWN }

#[prebindgen]
pub fn z_encoding_text_csv() -> ZEncoding { ZEncoding::TEXT_CSV }

#[prebindgen]
pub fn z_encoding_application_sql() -> ZEncoding { ZEncoding::APPLICATION_SQL }

#[prebindgen]
pub fn z_encoding_application_coap_payload() -> ZEncoding { ZEncoding::APPLICATION_COAP_PAYLOAD }

#[prebindgen]
pub fn z_encoding_application_json_patch_json() -> ZEncoding { ZEncoding::APPLICATION_JSON_PATCH_JSON }

#[prebindgen]
pub fn z_encoding_application_json_seq() -> ZEncoding { ZEncoding::APPLICATION_JSON_SEQ }

#[prebindgen]
pub fn z_encoding_application_jsonpath() -> ZEncoding { ZEncoding::APPLICATION_JSONPATH }

#[prebindgen]
pub fn z_encoding_application_jwt() -> ZEncoding { ZEncoding::APPLICATION_JWT }

#[prebindgen]
pub fn z_encoding_application_mp4() -> ZEncoding { ZEncoding::APPLICATION_MP4 }

#[prebindgen]
pub fn z_encoding_application_soap_xml() -> ZEncoding { ZEncoding::APPLICATION_SOAP_XML }

#[prebindgen]
pub fn z_encoding_application_yang() -> ZEncoding { ZEncoding::APPLICATION_YANG }

#[prebindgen]
pub fn z_encoding_audio_aac() -> ZEncoding { ZEncoding::AUDIO_AAC }

#[prebindgen]
pub fn z_encoding_audio_flac() -> ZEncoding { ZEncoding::AUDIO_FLAC }

#[prebindgen]
pub fn z_encoding_audio_mp4() -> ZEncoding { ZEncoding::AUDIO_MP4 }

#[prebindgen]
pub fn z_encoding_audio_ogg() -> ZEncoding { ZEncoding::AUDIO_OGG }

#[prebindgen]
pub fn z_encoding_audio_vorbis() -> ZEncoding { ZEncoding::AUDIO_VORBIS }

#[prebindgen]
pub fn z_encoding_video_h261() -> ZEncoding { ZEncoding::VIDEO_H261 }

#[prebindgen]
pub fn z_encoding_video_h263() -> ZEncoding { ZEncoding::VIDEO_H263 }

#[prebindgen]
pub fn z_encoding_video_h264() -> ZEncoding { ZEncoding::VIDEO_H264 }

#[prebindgen]
pub fn z_encoding_video_h265() -> ZEncoding { ZEncoding::VIDEO_H265 }

#[prebindgen]
pub fn z_encoding_video_h266() -> ZEncoding { ZEncoding::VIDEO_H266 }

#[prebindgen]
pub fn z_encoding_video_mp4() -> ZEncoding { ZEncoding::VIDEO_MP4 }

#[prebindgen]
pub fn z_encoding_video_ogg() -> ZEncoding { ZEncoding::VIDEO_OGG }

#[prebindgen]
pub fn z_encoding_video_raw() -> ZEncoding { ZEncoding::VIDEO_RAW }

#[prebindgen]
pub fn z_encoding_video_vp8() -> ZEncoding { ZEncoding::VIDEO_VP8 }

#[prebindgen]
pub fn z_encoding_video_vp9() -> ZEncoding { ZEncoding::VIDEO_VP9 }
