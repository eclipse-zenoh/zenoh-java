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

use prebindgen_proc_macro::prebindgen;

/// Flat sample shape carried across the FFI boundary.
///
/// Field order matches the positional arguments of the legacy
/// `JNISubscriberCallback.run(...)` Kotlin interface so the auto-generated
/// `Sample` data class constructor is a drop-in replacement for the prior
/// 11-arg method.
#[prebindgen]
#[derive(Debug, Clone)]
pub struct Sample {
    pub key_expr: String,
    pub payload: Vec<u8>,
    pub encoding_id: i64,
    pub encoding_schema: Option<String>,
    pub kind: i64,
    pub timestamp_ntp64: i64,
    pub timestamp_is_valid: bool,
    pub attachment: Option<Vec<u8>>,
    pub express: bool,
    pub priority: i64,
    pub congestion_control: i64,
}

impl From<&zenoh::sample::Sample> for Sample {
    /// Build a flat [`Sample`] from a zenoh `Sample`. Mirrors the field
    /// extraction that `process_kotlin_sample_callback` previously did
    /// inline before the JNI call.
    fn from(sample: &zenoh::sample::Sample) -> Self {
        let encoding = sample.encoding();
        let encoding_id = encoding.id() as i64;
        let encoding_schema = encoding
            .schema()
            .and_then(|schema| String::from_utf8(schema.to_vec()).ok());

        let (timestamp_ntp64, timestamp_is_valid) = sample
            .timestamp()
            .map(|t| (t.get_time().as_u64() as i64, true))
            .unwrap_or((0, false));

        let attachment = sample
            .attachment()
            .map(|att| att.to_bytes().into_owned());

        Sample {
            key_expr: sample.key_expr().to_string(),
            payload: sample.payload().to_bytes().into_owned(),
            encoding_id,
            encoding_schema,
            kind: sample.kind() as i64,
            timestamp_ntp64,
            timestamp_is_valid,
            attachment,
            express: sample.express(),
            priority: sample.priority() as i64,
            congestion_control: sample.congestion_control() as i64,
        }
    }
}
