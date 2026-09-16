//! Application of `key=value` overrides from [`AgentEditRequest::set_values`]
//! to the scalar fields of an [`AgentConfig`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::services::AgentConfig;

use super::edit::AgentEditRequest;
use super::{AgentConfigAuthoringService, ConfigAuthoringError};

impl AgentConfigAuthoringService {
    pub fn apply_set_value_changes(
        agent: &mut AgentConfig,
        request: &AgentEditRequest,
        changes: &mut Vec<String>,
    ) -> Result<(), ConfigAuthoringError> {
        for set_value in &request.set_values {
            let Some((key, value)) = set_value.split_once('=') else {
                return Err(ConfigAuthoringError::InvalidSetFormat(set_value.clone()));
            };
            apply_set_value(agent, key, value)?;
            changes.push(format!("{key}: {value}"));
        }
        Ok(())
    }
}

fn apply_set_value(
    agent: &mut AgentConfig,
    key: &str,
    value: &str,
) -> Result<(), ConfigAuthoringError> {
    match key {
        "card.displayName" | "card.display_name" => {
            value.clone_into(&mut agent.card.display_name);
        },
        "card.description" => {
            value.clone_into(&mut agent.card.description);
        },
        "card.version" => {
            value.clone_into(&mut agent.card.version);
        },
        "endpoint" => {
            value.clone_into(&mut agent.endpoint);
        },
        "is_primary" => {
            agent.is_primary = parse_bool(key, value)?;
        },
        "default" => {
            agent.default = parse_bool(key, value)?;
        },
        "dev_only" => {
            agent.dev_only = parse_bool(key, value)?;
        },
        _ => {
            return Err(ConfigAuthoringError::UnknownSetKey(key.to_owned()));
        },
    }
    Ok(())
}

fn parse_bool(key: &str, value: &str) -> Result<bool, ConfigAuthoringError> {
    value
        .parse()
        .map_err(|_e| ConfigAuthoringError::InvalidBoolean {
            key: key.to_owned(),
            value: value.to_owned(),
        })
}
