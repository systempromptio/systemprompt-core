//! Marketplace client defaults: what a manifest offers, and writing it into the
//! global settings file where the user has not set the key.
//!
//! Each key takes the first marketplace's value. The default `model` is written
//! as the bare id the engine lists, so a `<provider>/` prefix is dropped. The
//! default theme is the first marketplace whose default names one of its
//! themes, offered only when that theme's file is projected. A default the
//! engine's settings schema refuses is skipped with a warning; a settings file
//! that is not a JSONC object leaves every default unapplied.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Map, Value};
use systemprompt_device::protocol::manifest::Manifest;
use systemprompt_device::secure::read_optional;
use systemprompt_engine::config::{ConfigError, Layout, Paths, update_global_setting};

use crate::projection::error::ProjectionError;
use crate::projection::model::{BundleFile, DefaultTheme};

const MODEL_KEY: &str = "model";

// JSON: marketplace defaults are settings-file values, written verbatim by key.
pub(crate) fn project(manifest: &Manifest) -> Map<String, Value> {
    let mut out = Map::new();
    for settings in manifest.marketplace_settings() {
        for (key, value) in &settings.defaults {
            if out.contains_key(key) {
                continue;
            }
            let value = if key.as_str() == MODEL_KEY {
                bare_model(value)
            } else {
                value.clone()
            };
            out.insert(key.clone(), value);
        }
    }
    out
}

pub(crate) fn theme(manifest: &Manifest, files: &[BundleFile]) -> Option<DefaultTheme> {
    let settings = manifest
        .marketplace_settings()
        .iter()
        .find(|settings| settings.default_theme_ref().is_some())?;
    let theme = settings.default_theme_ref()?;
    let path = format!("themes/{}.json", theme.name);
    files
        .iter()
        .any(|file| file.path == path)
        .then(|| DefaultTheme {
            name: theme.name.clone(),
            mode: settings.theme_mode,
        })
}

// JSON: marketplace defaults are settings-file values, written verbatim by key.
pub(crate) fn write(
    paths: &Paths,
    layout: &Layout,
    defaults: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> Result<(), ProjectionError> {
    if defaults.is_empty() {
        return Ok(());
    }
    let file = layout.global_settings(paths);
    let current =
        read_optional(&file).map_err(ProjectionError::secure("read the global settings file"))?;
    let chosen = match current {
        None => Map::new(),
        Some(bytes) => {
            let Some(chosen) = parse_settings(&bytes) else {
                warnings.push(
                    "marketplace defaults not applied: the global settings file is not a JSONC \
                     object"
                        .to_owned(),
                );
                return Ok(());
            };
            chosen
        },
    };
    for (key, value) in defaults
        .iter()
        .filter(|(key, _)| !chosen.contains_key(*key))
    {
        match update_global_setting(paths, layout, &[key.as_str()], Some(value)) {
            Ok(changed) => tracing::debug!(key = %key, changed, "marketplace default applied"),
            Err(error @ (ConfigError::Read { .. } | ConfigError::Write { .. })) => {
                return Err(ProjectionError::Settings(error));
            },
            Err(error) => {
                tracing::warn!(key = %key, %error, "marketplace default refused by the settings schema");
                warnings.push(format!(
                    "marketplace default {key:?} not applied: the engine refuses its value"
                ));
            },
        }
    }
    Ok(())
}

// JSON: the user's settings file is free-form JSONC; only its top-level keys
// are read.
fn parse_settings(bytes: &[u8]) -> Option<Map<String, Value>> {
    let text = std::str::from_utf8(bytes).ok()?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if text.trim().is_empty() {
        return Some(Map::new());
    }
    match jsonc_parser::parse_to_serde_value::<Value>(text, &jsonc_parser::ParseOptions::default())
    {
        Ok(Value::Object(map)) => Some(map),
        Ok(Value::Null) => Some(Map::new()),
        Ok(_) | Err(_) => None,
    }
}

// JSON: a settings-file value; only a string model id is rewritten.
fn bare_model(value: &Value) -> Value {
    match value.as_str().and_then(|model| model.rsplit_once('/')) {
        Some((_, id)) if !id.is_empty() => Value::String(id.to_owned()),
        _ => value.clone(),
    }
}
