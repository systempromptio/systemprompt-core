//! Agent identity newtypes: opaque [`AgentId`] (UUID-backed), validated
//! [`AgentName`] (non-empty, reserves `"unknown"`), and
//! [`ExternalAgentId`] for off-platform "super-agents" (Claude Desktop,
//! Codex CLI, Claude Code) that connect via the bridge binary.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(AgentId, generate, schema);
crate::define_id!(ExternalAgentId, non_empty);

use crate::error::IdValidationError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, schemars::JsonSchema)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(feature = "sqlx", sqlx(transparent))]
#[serde(transparent)]
pub struct AgentName(String);

impl AgentName {
    pub fn try_new(name: impl Into<String>) -> Result<Self, IdValidationError> {
        let name = name.into();
        if name.is_empty() {
            return Err(IdValidationError::empty("AgentName"));
        }
        if name.eq_ignore_ascii_case("unknown") {
            return Err(IdValidationError::invalid(
                "AgentName",
                "'unknown' is reserved for error detection",
            ));
        }
        Ok(Self(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn system() -> Self {
        Self("system".to_owned())
    }

    pub fn bridge() -> Self {
        Self("bridge".to_owned())
    }

    // Why: placeholder for artifact metadata built before a request context
    // exists; `with_request` replaces it, and "unset" is distinguishable from
    // the reserved "unknown" that `try_new` rejects.
    pub fn unset() -> Self {
        Self("unset".to_owned())
    }
}

crate::__define_id_validated_conversions!(AgentName);
crate::__define_id_common!(AgentName);
