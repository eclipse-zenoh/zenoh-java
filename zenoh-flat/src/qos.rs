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

/// Flat mirror of [`zenoh::qos::Priority`]. `#[repr(i32)]` so the
/// auto-generated `as jni::sys::jint` encode in the JNI back-end is a
/// no-op cast, and `TryFrom<i32>` round-trips the same discriminant
/// values that ship over the wire.
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

impl TryFrom<i32> for Priority {
    type Error = String;

    fn try_from(v: i32) -> Result<Self, Self::Error> {
        Ok(match v {
            1 => Priority::RealTime,
            2 => Priority::InteractiveHigh,
            3 => Priority::InteractiveLow,
            4 => Priority::DataHigh,
            5 => Priority::Data,
            6 => Priority::DataLow,
            7 => Priority::Background,
            _ => return Err(format!("invalid Priority discriminant: {}", v)),
        })
    }
}

impl From<zenoh::qos::Priority> for Priority {
    fn from(p: zenoh::qos::Priority) -> Self {
        match p {
            zenoh::qos::Priority::RealTime => Priority::RealTime,
            zenoh::qos::Priority::InteractiveHigh => Priority::InteractiveHigh,
            zenoh::qos::Priority::InteractiveLow => Priority::InteractiveLow,
            zenoh::qos::Priority::DataHigh => Priority::DataHigh,
            zenoh::qos::Priority::Data => Priority::Data,
            zenoh::qos::Priority::DataLow => Priority::DataLow,
            zenoh::qos::Priority::Background => Priority::Background,
        }
    }
}

impl From<Priority> for zenoh::qos::Priority {
    fn from(p: Priority) -> Self {
        match p {
            Priority::RealTime => zenoh::qos::Priority::RealTime,
            Priority::InteractiveHigh => zenoh::qos::Priority::InteractiveHigh,
            Priority::InteractiveLow => zenoh::qos::Priority::InteractiveLow,
            Priority::DataHigh => zenoh::qos::Priority::DataHigh,
            Priority::Data => zenoh::qos::Priority::Data,
            Priority::DataLow => zenoh::qos::Priority::DataLow,
            Priority::Background => zenoh::qos::Priority::Background,
        }
    }
}
