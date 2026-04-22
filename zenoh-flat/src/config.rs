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

use crate::zerror;
use zenoh::config::Config;

/// Create a Zenoh configuration from a JSON string.
pub fn create_config_from_json(json: &str) -> crate::errors::ZResult<Config> {
    let mut deserializer = json5::Deserializer::from_str(json).map_err(|err| zerror!(err))?;
    Config::from_deserializer(&mut deserializer).map_err(|err| match err {
        Ok(c) => zerror!("Invalid configuration: {}", c),
        Err(e) => zerror!("JSON error: {}", e),
    })
}

/// Create a Zenoh configuration from a YAML string.
pub fn create_config_from_yaml(yaml: &str) -> crate::errors::ZResult<Config> {
    let deserializer = serde_yaml::Deserializer::from_str(yaml);
    Config::from_deserializer(deserializer).map_err(|err| match err {
        Ok(c) => zerror!("Invalid configuration: {}", c),
        Err(e) => zerror!("YAML error: {}", e),
    })
}
