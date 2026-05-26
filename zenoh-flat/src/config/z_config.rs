use crate::Error;
use crate::ZConfig;
use prebindgen_proc_macro::prebindgen;

/// Build a default configuration.
#[prebindgen]
pub fn z_config_default() -> ZConfig {
    ZConfig::default()
}

/// Load a configuration from a file path. The file extension determines
/// the format (JSON, JSON5, or YAML).
#[prebindgen]
pub fn z_config_from_file(path: &str) -> Result<ZConfig, Error> {
    Ok(ZConfig::from_file(path)?)
}

/// Parse a configuration from a JSON-formatted string. JSON is a subset
/// of JSON5, so routing through the JSON5 deserializer is sufficient.
#[prebindgen]
pub fn z_config_from_json(s: &str) -> Result<ZConfig, Error> {
    z_config_from_json5(s)
}

/// Parse a configuration from a JSON5-formatted string.
#[prebindgen]
pub fn z_config_from_json5(s: &str) -> Result<ZConfig, Error> {
    let mut deserializer = json5::Deserializer::from_str(s).map_err(|e| Error {
        message: format!("JSON error: {e}"),
    })?;
    ZConfig::from_deserializer(&mut deserializer).map_err(|err| match err {
        Ok(c) => Error {
            message: format!("Invalid configuration: {c}"),
        },
        Err(e) => Error {
            message: format!("JSON error: {e}"),
        },
    })
}

/// Parse a configuration from a YAML-formatted string.
#[prebindgen]
pub fn z_config_from_yaml(s: &str) -> Result<ZConfig, Error> {
    let deserializer = serde_yaml::Deserializer::from_str(s);
    ZConfig::from_deserializer(deserializer).map_err(|err| match err {
        Ok(c) => Error {
            message: format!("Invalid configuration: {c}"),
        },
        Err(e) => Error {
            message: format!("YAML error: {e}"),
        },
    })
}

/// Return the JSON value associated with `key` in the configuration.
#[prebindgen]
pub fn z_config_get_json(c: &ZConfig, key: &str) -> Result<String, Error> {
    Ok(c.get_json(key)?)
}

/// Insert a JSON5-formatted value at `key` in the configuration.
#[prebindgen]
pub fn z_config_insert_json5(c: &mut ZConfig, key: &str, value: &str) -> Result<(), Error> {
    c.insert_json5(key, value)?;
    Ok(())
}
