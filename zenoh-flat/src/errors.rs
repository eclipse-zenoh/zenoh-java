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

use std::fmt;

#[macro_export]
macro_rules! zerror {
    ($arg:expr) => {
        $crate::errors::ZError($arg.to_string())
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::errors::ZError(format!($fmt, $($arg)*))
    };
}

pub(crate) type ZResult<T> = core::result::Result<T, ZError>;

#[derive(Debug)]
pub struct ZError(pub String);

impl fmt::Display for ZError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
