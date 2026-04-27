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

//! Flat mirrors of the zenoh-ext advanced publisher/subscriber configs.
//!
//! Marked with `#[prebindgen]` so the JNI binding generator in
//! [`crate::jni_converter`] can emit a matching Kotlin `data class` and a
//! Rust JObject decoder for each one. The `TryFrom` impls encapsulate the
//! flat → semantic conversion (enum decoding happens already in the
//! auto-generated decoder, so these impls only deal with size/sign
//! adjustments and the "flag fields decide which builder method to call"
//! semantics).

use std::time::Duration;

use zenoh::qos::{CongestionControl, Priority};

use crate::errors::{ZError, ZResult};
use crate::zerror;

/// Flat cache configuration for an [`zenoh_ext::AdvancedPublisher`].
#[prebindgen_proc_macro::prebindgen]
#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub max_samples: i64,
    pub replies_priority: Priority,
    pub replies_congestion_control: CongestionControl,
    pub replies_is_express: bool,
}

/// Flat history configuration for an [`zenoh_ext::AdvancedSubscriber`].
///
/// `max_samples <= 0` and `max_age_seconds <= 0.0` mean "unlimited".
#[prebindgen_proc_macro::prebindgen]
#[derive(Debug, Clone)]
pub struct HistoryConfig {
    pub detect_late_publishers: bool,
    pub max_samples: i64,
    pub max_age_seconds: f64,
}

/// Flat miss-detection configuration for an [`zenoh_ext::AdvancedPublisher`].
///
/// When `enable_heartbeat` is `true`, the heartbeat is configured with
/// `period_ms` — sporadic if `is_sporadic`, regular otherwise. When
/// `enable_heartbeat` is `false`, the other fields are ignored and
/// [`zenoh_ext::MissDetectionConfig::default`] is used as-is.
#[prebindgen_proc_macro::prebindgen]
#[derive(Debug, Clone)]
pub struct MissDetectionConfig {
    pub enable_heartbeat: bool,
    pub is_sporadic: bool,
    pub period_ms: i64,
}

/// Flat recovery configuration for an [`zenoh_ext::AdvancedSubscriber`].
///
/// When `is_heartbeat` is `true`, `period_ms` is ignored; otherwise
/// `period_ms` is the `periodic_queries` period in milliseconds.
#[prebindgen_proc_macro::prebindgen]
#[derive(Debug, Clone)]
pub struct RecoveryConfig {
    pub is_heartbeat: bool,
    pub period_ms: i64,
}

impl TryFrom<CacheConfig> for zenoh_ext::CacheConfig {
    type Error = ZError;

    fn try_from(c: CacheConfig) -> ZResult<Self> {
        let max_samples: usize = c
            .max_samples
            .try_into()
            .map_err(|e: std::num::TryFromIntError| zerror!("CacheConfig.max_samples: {}", e))?;
        let replies = zenoh_ext::RepliesConfig::default()
            .priority(c.replies_priority)
            .congestion_control(c.replies_congestion_control)
            .express(c.replies_is_express);
        Ok(zenoh_ext::CacheConfig::default()
            .max_samples(max_samples)
            .replies_config(replies))
    }
}

impl TryFrom<HistoryConfig> for zenoh_ext::HistoryConfig {
    type Error = ZError;

    fn try_from(c: HistoryConfig) -> ZResult<Self> {
        let mut cfg = zenoh_ext::HistoryConfig::default();
        if c.detect_late_publishers {
            cfg = cfg.detect_late_publishers();
        }
        if c.max_samples > 0 {
            let n: usize = c.max_samples.try_into().map_err(
                |e: std::num::TryFromIntError| zerror!("HistoryConfig.max_samples: {}", e),
            )?;
            cfg = cfg.max_samples(n);
        }
        if c.max_age_seconds > 0.0 {
            cfg = cfg.max_age(c.max_age_seconds);
        }
        Ok(cfg)
    }
}

impl TryFrom<MissDetectionConfig> for zenoh_ext::MissDetectionConfig {
    type Error = ZError;

    fn try_from(c: MissDetectionConfig) -> ZResult<Self> {
        let mut cfg = zenoh_ext::MissDetectionConfig::default();
        if c.enable_heartbeat {
            let period_ms: u64 = c.period_ms.try_into().map_err(
                |e: std::num::TryFromIntError| zerror!("MissDetectionConfig.period_ms: {}", e),
            )?;
            let dur = Duration::from_millis(period_ms);
            cfg = if c.is_sporadic {
                cfg.sporadic_heartbeat(dur)
            } else {
                cfg.heartbeat(dur)
            };
        }
        Ok(cfg)
    }
}

impl TryFrom<RecoveryConfig> for zenoh_ext::RecoveryConfig {
    type Error = ZError;

    fn try_from(c: RecoveryConfig) -> ZResult<Self> {
        if c.is_heartbeat {
            Ok(zenoh_ext::RecoveryConfig::default().heartbeat())
        } else {
            let period_ms: u64 = c.period_ms.try_into().map_err(
                |e: std::num::TryFromIntError| zerror!("RecoveryConfig.period_ms: {}", e),
            )?;
            Ok(zenoh_ext::RecoveryConfig::default().periodic_queries(Duration::from_millis(period_ms)))
        }
    }
}
