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

//! Flat QoS types for cross-FFI marshalling.
//!
//! Each enum here mirrors its upstream `zenoh::qos::*` counterpart but is
//! owned by `zenoh-flat` so the discriminant values and variant set we
//! expose to bindings can drift independently from upstream. The
//! `From`/`Into` impls below are the *manual* shim between the flat enum
//! and the upstream one — change them when upstream renames or
//! renumbers a variant.

use prebindgen_proc_macro::prebindgen;
use zenoh::qos::{
    CongestionControl as ZCongestionControl, Priority as ZPriority, Reliability as ZReliability,
};

/// Flat mirror of [`zenoh::qos::Priority`]. `#[repr(i32)]` so the
/// auto-generated JNI converters round-trip the discriminant values
/// over the wire: the framework derives the `jint → variant` decode and
/// `variant as jint` encode straight from these discriminants, so no
/// `TryFrom<i32>` is needed here — only the semantic `From`/`Into` shim
/// to upstream below.
#[prebindgen]
#[repr(i32)]
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Priority {
    RealTime = 1,
    InteractiveHigh = 2,
    InteractiveLow = 3,
    DataHigh = 4,
    #[default]
    Data = 5,
    DataLow = 6,
    Background = 7,
}

impl From<ZPriority> for Priority {
    fn from(p: ZPriority) -> Self {
        match p {
            ZPriority::RealTime => Priority::RealTime,
            ZPriority::InteractiveHigh => Priority::InteractiveHigh,
            ZPriority::InteractiveLow => Priority::InteractiveLow,
            ZPriority::DataHigh => Priority::DataHigh,
            ZPriority::Data => Priority::Data,
            ZPriority::DataLow => Priority::DataLow,
            ZPriority::Background => Priority::Background,
        }
    }
}

impl From<Priority> for ZPriority {
    fn from(p: Priority) -> Self {
        match p {
            Priority::RealTime => ZPriority::RealTime,
            Priority::InteractiveHigh => ZPriority::InteractiveHigh,
            Priority::InteractiveLow => ZPriority::InteractiveLow,
            Priority::DataHigh => ZPriority::DataHigh,
            Priority::Data => ZPriority::Data,
            Priority::DataLow => ZPriority::DataLow,
            Priority::Background => ZPriority::Background,
        }
    }
}

/// Flat mirror of [`zenoh::qos::CongestionControl`]. Discriminants are
/// the wire values the binding sends; `#[repr(i32)]` keeps the JNI
/// `as jint` round-trip a no-op cast.
#[prebindgen]
#[repr(i32)]
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Hash)]
pub enum CongestionControl {
    #[default]
    Drop = 0,
    Block = 1,
}

impl From<CongestionControl> for ZCongestionControl {
    fn from(c: CongestionControl) -> Self {
        match c {
            CongestionControl::Drop => ZCongestionControl::Drop,
            CongestionControl::Block => ZCongestionControl::Block,
        }
    }
}

/// Flat mirror of [`zenoh::qos::Reliability`].
#[prebindgen]
#[repr(i32)]
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Reliability {
    BestEffort = 0,
    #[default]
    Reliable = 1,
}

impl From<Reliability> for ZReliability {
    fn from(r: Reliability) -> Self {
        match r {
            Reliability::BestEffort => ZReliability::BestEffort,
            Reliability::Reliable => ZReliability::Reliable,
        }
    }
}
