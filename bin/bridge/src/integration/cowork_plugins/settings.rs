//! Pure JSON manipulation for `cowork_settings.json`'s `enabledPlugins` map.
//! Foreign keys (the user's own choices) are preserved verbatim.

use serde_json::{Map, Value};

use super::CoworkPluginsError;

const KEY_ENABLED: &str = "enabledPlugins";

#[must_use]
pub fn enabled_plugins_key(plugin: &str, marketplace: &str) -> String {
    format!("{plugin}@{marketplace}")
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SettingsReport {
    pub set: bool,
    pub already: bool,
}

pub fn parse_settings(bytes: &[u8]) -> Result<Map<String, Value>, CoworkPluginsError> {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(Map::new());
    }
    match serde_json::from_slice::<Value>(bytes)? {
        Value::Object(root) => Ok(root),
        _ => Err(CoworkPluginsError::RootShape),
    }
}

pub fn render_settings(root: &Map<String, Value>) -> Result<Vec<u8>, CoworkPluginsError> {
    serde_json::to_vec_pretty(&Value::Object(root.clone())).map_err(CoworkPluginsError::JsonParse)
}

pub fn enable_plugin(
    root: &mut Map<String, Value>,
    plugin: &str,
    marketplace: &str,
) -> Result<SettingsReport, CoworkPluginsError> {
    let key = enabled_plugins_key(plugin, marketplace);
    let map = ensure_enabled_map(root)?;
    let mut report = SettingsReport::default();
    if matches!(map.get(&key), Some(Value::Bool(true))) {
        report.already = true;
    } else {
        map.insert(key, Value::Bool(true));
        report.set = true;
    }
    Ok(report)
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "Result-returning parity with enable_plugin for the symmetric enable/disable API"
)]
pub fn disable_plugin(
    root: &mut Map<String, Value>,
    plugin: &str,
    marketplace: &str,
) -> Result<bool, CoworkPluginsError> {
    let key = enabled_plugins_key(plugin, marketplace);
    let Some(map) = root.get_mut(KEY_ENABLED).and_then(Value::as_object_mut) else {
        return Ok(false);
    };
    Ok(map.remove(&key).is_some())
}

fn ensure_enabled_map(
    root: &mut Map<String, Value>,
) -> Result<&mut Map<String, Value>, CoworkPluginsError> {
    match root
        .entry(KEY_ENABLED.to_owned())
        .or_insert_with(|| Value::Object(Map::new()))
    {
        Value::Object(m) => Ok(m),
        _ => Err(CoworkPluginsError::ItemsShape { key: KEY_ENABLED }),
    }
}
