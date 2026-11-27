//! Scenario-file loading shared by the scripted types.

use std::path::Path;

use serde::de::DeserializeOwned;

use crate::error::ScriptedInstanceError;

pub(crate) fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, ScriptedInstanceError> {
    let text = std::fs::read_to_string(path).map_err(|source| ScriptedInstanceError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| ScriptedInstanceError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn parse_json<T: DeserializeOwned>(json: &str) -> Result<T, ScriptedInstanceError> {
    serde_json::from_str(json).map_err(ScriptedInstanceError::ParseInline)
}
