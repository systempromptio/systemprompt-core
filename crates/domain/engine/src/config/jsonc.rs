//! JSONC reading, deep merging, and comment-preserving edits of the global
//! settings file.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use jsonc_parser::ParseOptions;
use jsonc_parser::cst::{CstInputValue, CstObject, CstRootNode};
use serde_json::{Map, Value};

use crate::config::error::ConfigError;
use crate::config::fsio::write_atomic;
use crate::config::paths::{Layout, Paths};

pub(crate) fn options() -> ParseOptions {
    ParseOptions::default()
}

// JSON: a settings file is a free-form JSON object until it is merged and
// deserialized.
pub(crate) fn parse_object(path: &Path, text: &str) -> Result<Map<String, Value>, ConfigError> {
    let value: Value = jsonc_parser::parse_to_serde_value(text, &options())
        .map_err(|e| ConfigError::jsonc(path, e))?;
    match value {
        Value::Null => Ok(Map::new()),
        Value::Object(map) => Ok(map),
        _ => Err(ConfigError::NotAnObject {
            path: path.to_path_buf(),
        }),
    }
}

// JSON: settings layers merge key by key before they are typed.
pub(crate) fn merge_value(base: &mut Value, overlay: Value) {
    match (base, overlay) {
        (Value::Object(base), Value::Object(overlay)) => {
            for (key, value) in overlay {
                match base.get_mut(&key) {
                    Some(slot) => merge_value(slot, value),
                    None => {
                        base.insert(key, value);
                    },
                }
            }
        },
        (slot, value) => *slot = value,
    }
}

// JSON: a settings value converted for the comment-preserving editor.
fn input(value: &Value) -> CstInputValue {
    match value {
        Value::Null => CstInputValue::Null,
        Value::Bool(flag) => CstInputValue::Bool(*flag),
        Value::Number(number) => CstInputValue::Number(number.to_string()),
        Value::String(text) => CstInputValue::String(text.clone()),
        Value::Array(items) => CstInputValue::Array(items.iter().map(input).collect()),
        Value::Object(map) => CstInputValue::Object(
            map.iter()
                .map(|(key, value)| (key.clone(), input(value)))
                .collect(),
        ),
    }
}

fn descend(root: &CstRootNode, parents: &[&str], create: bool) -> Option<CstObject> {
    if create {
        let mut object = root.object_value_or_set();
        for key in parents {
            object = object.object_value_or_set(key);
        }
        return Some(object);
    }
    let mut object = root.object_value()?;
    for key in parents {
        object = object.object_value(key)?;
    }
    Some(object)
}

// JSON: the value written at a settings key path; `None` removes the key.
pub(crate) fn edit_text(
    path: &Path,
    text: &str,
    key: &[&str],
    value: Option<&Value>,
) -> Result<String, ConfigError> {
    let Some((last, parents)) = key.split_last() else {
        return Err(ConfigError::EmptyKey);
    };
    let source = if text.trim().is_empty() { "{}\n" } else { text };
    let root = CstRootNode::parse(source, &options()).map_err(|e| ConfigError::jsonc(path, e))?;
    match value {
        Some(value) => {
            if let Some(object) = descend(&root, parents, true) {
                match object.get(last) {
                    Some(prop) => prop.set_value(input(value)),
                    None => {
                        object.append(last, input(value));
                    },
                }
            }
        },
        None => {
            if let Some(prop) = descend(&root, parents, false).and_then(|object| object.get(last)) {
                prop.remove();
            }
        },
    }
    Ok(root.to_string())
}

// JSON: the value written at a settings key path; `None` removes the key.
pub fn update_global_setting(
    paths: &Paths,
    layout: &Layout,
    key: &[&str],
    value: Option<&Value>,
) -> Result<bool, ConfigError> {
    let file = layout.global_settings(paths);
    let before = match std::fs::read_to_string(&file) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(source) => return Err(ConfigError::Read { path: file, source }),
    };
    let after = edit_text(&file, &before, key, value)?;
    crate::config::settings::validate_text(&file, &after, &paths.home)?;
    if after == before {
        return Ok(false);
    }
    write_atomic(&file, &after)?;
    let removed = value.is_none();
    tracing::info!(file = %file.display(), key = %key.join("."), removed, "global setting updated");
    Ok(true)
}
