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

use jni::JNIEnv;

#[macro_export]
macro_rules! throw_exception {
    ($env:expr, $err:expr) => {{
        let _ = <crate::errors::ZError as crate::errors::ThrowOnJvm>::throw_on_jvm(
            &$err,
            &mut $env,
        )
        .map_err(|err| {
            tracing::error!("Unable to throw exception: {}", err);
        });
    }};
}

pub(crate) type ZError = zenoh_flat::errors::ZError;
pub(crate) type ZResult<T> = core::result::Result<T, ZError>;

pub(crate) trait ThrowOnJvm {
    fn throw_on_jvm(&self, env: &mut JNIEnv) -> ZResult<()>;
}

impl ThrowOnJvm for ZError {
    fn throw_on_jvm(&self, env: &mut JNIEnv) -> ZResult<()> {
        let exception_class = env
            .find_class("io/zenoh/exceptions/ZError")
            .map_err(|err| zerror!("Failed to retrieve exception class: {}", err))?;
        env.throw_new(exception_class, self.to_string())
            .map_err(|err| zerror!("Failed to throw exception: {}", err))
    }
}
