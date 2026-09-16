//! Foreign-key-preserving JSON reads and atomic writes for open-schema host
//! files.
//!
//! Covers the Claude CLI registry and `opencode.json`. A malformed file, a
//! non-object root, or a bridge-owned key holding a foreign shape is an error;
//! the file is never rewritten to fit.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::Path;

use serde_json::{Map, Value};

use crate::fsutil;
use crate::host_sync::ApplyError;
use crate::integration::config_read::ForeignShape;

fn io_err(context: impl Into<String>, source: std::io::Error) -> ApplyError {
    ApplyError::Io {
        context: context.into(),
        source,
    }
}

pub fn read_optional_object(path: &Path) -> Result<Option<Map<String, Value>>, ApplyError> {
    let Some(text) =
        fsutil::read_optional(path).map_err(|e| io_err(format!("read {}", path.display()), e))?
    else {
        return Ok(None);
    };
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    if text.trim().is_empty() {
        return Ok(Some(Map::new()));
    }
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(m)) => Ok(Some(m)),
        Ok(_) => Err(io_err(
            format!(
                "{} is not a JSON object; refusing to overwrite",
                path.display()
            ),
            std::io::Error::other("unexpected JSON root"),
        )),
        Err(e) => Err(io_err(
            format!("parse {}; refusing to overwrite", path.display()),
            std::io::Error::other(e),
        )),
    }
}

pub(crate) fn read_json_object(path: &Path) -> Result<Map<String, Value>, ApplyError> {
    Ok(read_optional_object(path)?.unwrap_or_default())
}

// JSON: open-schema host file; the bridge owns `key` but every other key is
// the user's and is carried through untouched.
pub fn object_entry<'a>(
    root: &'a mut Map<String, Value>,
    path: &Path,
    key: &'static str,
) -> Result<&'a mut Map<String, Value>, ForeignShape> {
    let slot = root.entry(key).or_insert_with(|| Value::Object(Map::new()));
    match slot {
        Value::Object(map) => Ok(map),
        other => Err(ForeignShape {
            path: path.display().to_string(),
            key: key.to_owned(),
            found: json_kind(other),
            expected: "an object",
        }),
    }
}

const fn json_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

pub(crate) fn write_json(path: &Path, value: &Value) -> Result<(), ApplyError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| io_err(format!("create {}", parent.display()), e))?;
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| ApplyError::Serialize {
        what: path.display().to_string(),
        source: e,
    })?;
    fsutil::atomic_write_0600(path, &bytes)
        .map_err(|e| io_err(format!("atomic_write {}", path.display()), e))
}
