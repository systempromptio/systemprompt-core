//! Reading a partial agent from a front matter block or an `agent.<name>`
//! settings object.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde_json::{Map, Value};

use super::tools::names;
use super::{AgentMode, AgentOverride, ToolFilter};
use crate::config::error::FieldError;
use crate::config::permission::Ruleset;

type Refused = (String, FieldError);

// JSON: one front matter or settings value read as text.
fn text(key: &str, value: &Value) -> Result<String, Refused> {
    match value {
        Value::String(text) => Ok(text.clone()),
        _ => Err((
            key.to_owned(),
            FieldError::Type {
                expected: "a string",
            },
        )),
    }
}

// JSON: one front matter or settings value read as a boolean.
fn flag(key: &str, value: &Value) -> Result<bool, Refused> {
    value.as_bool().ok_or_else(|| {
        (
            key.to_owned(),
            FieldError::Type {
                expected: "true or false",
            },
        )
    })
}

// JSON: the agent's `mode` value.
fn mode(key: &str, value: &Value) -> Result<AgentMode, Refused> {
    let mode = text(key, value)?;
    AgentMode::parse(&mode).ok_or_else(|| {
        let expected = "primary, subagent or all";
        let refused = FieldError::Value {
            value: mode.clone(),
            expected,
        };
        (key.to_owned(), refused)
    })
}

// JSON: the agent's `temperature` value.
fn temperature(key: &str, value: &Value) -> Result<f32, Refused> {
    let number = value.as_f64().ok_or_else(|| {
        (
            key.to_owned(),
            FieldError::Type {
                expected: "a number",
            },
        )
    })?;
    Ok(number as f32)
}

// JSON: one of the agent's limits: steps per turn or output tokens per step.
fn limit(key: &str, value: &Value) -> Result<u32, Refused> {
    value
        .as_u64()
        .and_then(|limit| u32::try_from(limit).ok())
        .ok_or_else(|| {
            (
                key.to_owned(),
                FieldError::Type {
                    expected: "a positive whole number",
                },
            )
        })
}

impl AgentOverride {
    // JSON: a front matter block or an `agent.<name>` settings object; unknown keys
    // go to `extra`.
    pub(crate) fn from_map(map: &Map<String, Value>, home: &Path) -> Result<Self, Refused> {
        let mut patch = Self::default();
        let mut denied: Vec<String> = Vec::new();
        for (key, value) in map {
            match key.as_str() {
                "name" => patch.name = Some(text(key, value)?),
                "description" => patch.description = Some(text(key, value)?),
                "prompt" => patch.prompt = Some(text(key, value)?),
                "model" => {
                    let model = text(key, value)?;
                    if model != "inherit" && !model.trim().is_empty() {
                        patch.model = Some(model);
                    }
                },
                "mode" => patch.mode = Some(mode(key, value)?),
                "temperature" => patch.temperature = Some(temperature(key, value)?),
                "steps" | "maxSteps" | "max_steps" => patch.steps = Some(limit(key, value)?),
                "maxOutput" | "max_output" => patch.max_output = Some(limit(key, value)?),
                "hidden" => patch.hidden = Some(flag(key, value)?),
                "disable" | "disabled" => patch.disable = flag(key, value)?,
                "tools" => {
                    let filter = ToolFilter::from_value(value).map_err(|e| (key.clone(), e))?;
                    patch.tools = Some(filter);
                },
                "disallowedTools" | "disallowed_tools" => {
                    denied.extend(names(value).map_err(|e| (key.clone(), e))?);
                },
                "permission" => {
                    let rules = Ruleset::from_value(value, home)
                        .map_err(|e| (key.clone(), FieldError::from(e)))?;
                    patch.permission.extend(&rules);
                },
                _ => {
                    patch.extra.insert(key.clone(), value.clone());
                },
            }
        }
        if !denied.is_empty() {
            patch
                .tools
                .get_or_insert_with(ToolFilter::default)
                .deny
                .extend(denied);
        }
        Ok(patch)
    }
}
