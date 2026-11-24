//! Single-owner rules for services config: one default agent, one owner of
//! the governance hooks and one of the judge hook.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::ServicesConfig;
use systemprompt_models::errors::{ServicesValidationError, ValidationSubject};

impl ServicesConfig {
    pub(crate) fn validate_single_default_agent(&self) -> Result<(), ServicesValidationError> {
        let default_agents: Vec<&str> = self
            .agents
            .iter()
            .filter_map(|(name, agent)| {
                if agent.default {
                    Some(name.as_str())
                } else {
                    None
                }
            })
            .collect();

        match default_agents.len() {
            0 | 1 => Ok(()),
            _ => Err(ServicesValidationError::business_rule(
                ValidationSubject::Global,
                format!(
                    "Multiple agents marked as default: {}. Only one agent can have 'default: true'",
                    default_agents.join(", ")
                ),
            )),
        }
    }

    pub(crate) fn validate_single_governance_hook_owner(
        &self,
    ) -> Result<(), ServicesValidationError> {
        let owners: Vec<&str> = self
            .plugins
            .values()
            .filter(|p| p.enabled && p.hooks.governance)
            .map(|p| p.id.as_str())
            .collect();

        match owners.len() {
            0 => {
                tracing::warn!(
                    "no enabled plugin sets 'hooks.governance: true' — governance hooks will \
                     not be installed, so no tool call will be checked"
                );
                Ok(())
            },
            1 => Ok(()),
            _ => Err(ServicesValidationError::business_rule(
                ValidationSubject::Global,
                format!(
                    "Multiple plugins set 'hooks.governance: true': {}. Hooks run session-globally, \
                 so at most one plugin may own them",
                    owners.join(", ")
                ),
            )),
        }
    }

    pub(crate) fn validate_single_judge_hook_owner(&self) -> Result<(), ServicesValidationError> {
        let owners: Vec<&str> = self
            .plugins
            .values()
            .filter(|p| p.enabled && p.hooks.judge)
            .map(|p| p.id.as_str())
            .collect();

        if let Some(orphan) = self
            .plugins
            .values()
            .find(|p| p.enabled && p.hooks.judge && !p.hooks.governance)
        {
            return Err(ServicesValidationError::business_rule(
                ValidationSubject::clause("Plugin", &orphan.id),
                "sets 'hooks.judge: true' without 'hooks.governance: true'. \
                 The judge consumes the track hook that only the governance owner installs",
            ));
        }

        match owners.len() {
            0 => {
                tracing::warn!(
                    "no enabled plugin sets 'hooks.judge: true' — conversations will not be \
                     scored"
                );
                Ok(())
            },
            1 => Ok(()),
            _ => Err(ServicesValidationError::business_rule(
                ValidationSubject::Global,
                format!(
                    "Multiple plugins set 'hooks.judge: true': {}. The judge has \
                 one owner",
                    owners.join(", ")
                ),
            )),
        }
    }
}
