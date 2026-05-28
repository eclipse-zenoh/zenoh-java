use super::z_encoding::{z_encoding_id, z_encoding_schema};
use crate::{Error, ZEncoding};
use prebindgen_proc_macro::prebindgen;
use zenoh::internal::buffers::ZSlice;

/// Value-class twin of [`ZEncoding`]. Carries decomposed fields so
/// bindings with expensive FFI boundaries can read id/schema without
/// round-tripping into native code.
#[prebindgen]
#[derive(Clone)]
pub struct Encoding {
    pub id: i32,
    pub schema: Option<String>,
}

impl From<&ZEncoding> for Encoding {
    fn from(e: &ZEncoding) -> Self {
        Self {
            id: z_encoding_id(e),
            schema: z_encoding_schema(e),
        }
    }
}

impl From<ZEncoding> for Encoding {
    fn from(e: ZEncoding) -> Self {
        Self::from(&e)
    }
}

impl TryFrom<Encoding> for ZEncoding {
    type Error = Error;
    fn try_from(e: Encoding) -> Result<Self, Error> {
        let id = u16::try_from(e.id).map_err(|err| Error {
            message: format!("Invalid encoding id {}: {err}", e.id),
        })?;
        let schema = e.schema.map(|s| ZSlice::from(s.into_bytes()));
        Ok(ZEncoding::new(id, schema))
    }
}

impl TryFrom<&Encoding> for ZEncoding {
    type Error = Error;
    fn try_from(e: &Encoding) -> Result<Self, Error> {
        ZEncoding::try_from(e.clone())
    }
}

/// Parse a textual encoding (e.g. `"text/plain"`, `"text/plain;utf-8"`,
/// `"my_encoding"`). Mirrors upstream `From<&str> for zenoh::bytes::Encoding`:
/// known names resolve to their canonical id; everything else is stored
/// verbatim in the schema field under the `CUSTOM_ENCODING_ID`.
impl From<String> for Encoding {
    fn from(s: String) -> Self {
        ZEncoding::from(s).into()
    }
}

impl From<&str> for Encoding {
    fn from(s: &str) -> Self {
        ZEncoding::from(s).into()
    }
}

/// Construct an [`Encoding`] by parsing its textual form. Uses upstream's
/// string parser (well-known names → canonical id; anything else → custom).
#[prebindgen]
pub fn encoding_from_string(s: String) -> Encoding {
    s.into()
}

/// Canonical display string for an [`Encoding`] (delegates to upstream's
/// `Display`, which renders predefined ids by name and unknown ids as
/// `unknown(<id>)`, with `;<schema>` appended when present).
#[prebindgen]
pub fn encoding_to_string(e: &Encoding) -> String {
    let ze: ZEncoding = ZEncoding::try_from(e)
        .ok()
        .expect("encoding id out of u16 range");
    ze.to_string()
}

// ── Predefined-constant accessors ─────────────────────────────────────────
// One #[prebindgen] fn per upstream `pub const`. Adding a new upstream
// constant = one new line here.

#[prebindgen]
pub fn encoding_zenoh_bytes() -> Encoding { (&ZEncoding::ZENOH_BYTES).into() }

#[prebindgen]
pub fn encoding_zenoh_string() -> Encoding { (&ZEncoding::ZENOH_STRING).into() }

#[prebindgen]
pub fn encoding_zenoh_serialized() -> Encoding { (&ZEncoding::ZENOH_SERIALIZED).into() }

#[prebindgen]
pub fn encoding_application_octet_stream() -> Encoding { (&ZEncoding::APPLICATION_OCTET_STREAM).into() }

#[prebindgen]
pub fn encoding_text_plain() -> Encoding { (&ZEncoding::TEXT_PLAIN).into() }

#[prebindgen]
pub fn encoding_application_json() -> Encoding { (&ZEncoding::APPLICATION_JSON).into() }

#[prebindgen]
pub fn encoding_text_json() -> Encoding { (&ZEncoding::TEXT_JSON).into() }

#[prebindgen]
pub fn encoding_application_cdr() -> Encoding { (&ZEncoding::APPLICATION_CDR).into() }

#[prebindgen]
pub fn encoding_application_cbor() -> Encoding { (&ZEncoding::APPLICATION_CBOR).into() }

#[prebindgen]
pub fn encoding_application_yaml() -> Encoding { (&ZEncoding::APPLICATION_YAML).into() }

#[prebindgen]
pub fn encoding_text_yaml() -> Encoding { (&ZEncoding::TEXT_YAML).into() }

#[prebindgen]
pub fn encoding_text_json5() -> Encoding { (&ZEncoding::TEXT_JSON5).into() }

#[prebindgen]
pub fn encoding_application_python_serialized_object() -> Encoding { (&ZEncoding::APPLICATION_PYTHON_SERIALIZED_OBJECT).into() }

#[prebindgen]
pub fn encoding_application_protobuf() -> Encoding { (&ZEncoding::APPLICATION_PROTOBUF).into() }

#[prebindgen]
pub fn encoding_application_java_serialized_object() -> Encoding { (&ZEncoding::APPLICATION_JAVA_SERIALIZED_OBJECT).into() }

#[prebindgen]
pub fn encoding_application_openmetrics_text() -> Encoding { (&ZEncoding::APPLICATION_OPENMETRICS_TEXT).into() }

#[prebindgen]
pub fn encoding_image_png() -> Encoding { (&ZEncoding::IMAGE_PNG).into() }

#[prebindgen]
pub fn encoding_image_jpeg() -> Encoding { (&ZEncoding::IMAGE_JPEG).into() }

#[prebindgen]
pub fn encoding_image_gif() -> Encoding { (&ZEncoding::IMAGE_GIF).into() }

#[prebindgen]
pub fn encoding_image_bmp() -> Encoding { (&ZEncoding::IMAGE_BMP).into() }

#[prebindgen]
pub fn encoding_image_webp() -> Encoding { (&ZEncoding::IMAGE_WEBP).into() }

#[prebindgen]
pub fn encoding_application_xml() -> Encoding { (&ZEncoding::APPLICATION_XML).into() }

#[prebindgen]
pub fn encoding_application_x_www_form_urlencoded() -> Encoding { (&ZEncoding::APPLICATION_X_WWW_FORM_URLENCODED).into() }

#[prebindgen]
pub fn encoding_text_html() -> Encoding { (&ZEncoding::TEXT_HTML).into() }

#[prebindgen]
pub fn encoding_text_xml() -> Encoding { (&ZEncoding::TEXT_XML).into() }

#[prebindgen]
pub fn encoding_text_css() -> Encoding { (&ZEncoding::TEXT_CSS).into() }

#[prebindgen]
pub fn encoding_text_javascript() -> Encoding { (&ZEncoding::TEXT_JAVASCRIPT).into() }

#[prebindgen]
pub fn encoding_text_markdown() -> Encoding { (&ZEncoding::TEXT_MARKDOWN).into() }

#[prebindgen]
pub fn encoding_text_csv() -> Encoding { (&ZEncoding::TEXT_CSV).into() }

#[prebindgen]
pub fn encoding_application_sql() -> Encoding { (&ZEncoding::APPLICATION_SQL).into() }

#[prebindgen]
pub fn encoding_application_coap_payload() -> Encoding { (&ZEncoding::APPLICATION_COAP_PAYLOAD).into() }

#[prebindgen]
pub fn encoding_application_json_patch_json() -> Encoding { (&ZEncoding::APPLICATION_JSON_PATCH_JSON).into() }

#[prebindgen]
pub fn encoding_application_json_seq() -> Encoding { (&ZEncoding::APPLICATION_JSON_SEQ).into() }

#[prebindgen]
pub fn encoding_application_jsonpath() -> Encoding { (&ZEncoding::APPLICATION_JSONPATH).into() }

#[prebindgen]
pub fn encoding_application_jwt() -> Encoding { (&ZEncoding::APPLICATION_JWT).into() }

#[prebindgen]
pub fn encoding_application_mp4() -> Encoding { (&ZEncoding::APPLICATION_MP4).into() }

#[prebindgen]
pub fn encoding_application_soap_xml() -> Encoding { (&ZEncoding::APPLICATION_SOAP_XML).into() }

#[prebindgen]
pub fn encoding_application_yang() -> Encoding { (&ZEncoding::APPLICATION_YANG).into() }

#[prebindgen]
pub fn encoding_audio_aac() -> Encoding { (&ZEncoding::AUDIO_AAC).into() }

#[prebindgen]
pub fn encoding_audio_flac() -> Encoding { (&ZEncoding::AUDIO_FLAC).into() }

#[prebindgen]
pub fn encoding_audio_mp4() -> Encoding { (&ZEncoding::AUDIO_MP4).into() }

#[prebindgen]
pub fn encoding_audio_ogg() -> Encoding { (&ZEncoding::AUDIO_OGG).into() }

#[prebindgen]
pub fn encoding_audio_vorbis() -> Encoding { (&ZEncoding::AUDIO_VORBIS).into() }

#[prebindgen]
pub fn encoding_video_h261() -> Encoding { (&ZEncoding::VIDEO_H261).into() }

#[prebindgen]
pub fn encoding_video_h263() -> Encoding { (&ZEncoding::VIDEO_H263).into() }

#[prebindgen]
pub fn encoding_video_h264() -> Encoding { (&ZEncoding::VIDEO_H264).into() }

#[prebindgen]
pub fn encoding_video_h265() -> Encoding { (&ZEncoding::VIDEO_H265).into() }

#[prebindgen]
pub fn encoding_video_h266() -> Encoding { (&ZEncoding::VIDEO_H266).into() }

#[prebindgen]
pub fn encoding_video_mp4() -> Encoding { (&ZEncoding::VIDEO_MP4).into() }

#[prebindgen]
pub fn encoding_video_ogg() -> Encoding { (&ZEncoding::VIDEO_OGG).into() }

#[prebindgen]
pub fn encoding_video_raw() -> Encoding { (&ZEncoding::VIDEO_RAW).into() }

#[prebindgen]
pub fn encoding_video_vp8() -> Encoding { (&ZEncoding::VIDEO_VP8).into() }

#[prebindgen]
pub fn encoding_video_vp9() -> Encoding { (&ZEncoding::VIDEO_VP9).into() }
