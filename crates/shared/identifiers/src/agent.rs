//! Agent identity newtypes: opaque [`AgentId`] (UUID-backed), validated
//! [`AgentName`] (non-empty, reserves `"unknown"`), and
//! [`ExternalAgentId`] for off-platform "super-agents" (Claude Desktop,
//! Codex CLI, Claude Code) that connect via the bridge binary.

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

    #[expect(
        clippy::expect_used,
        reason = "infallible constructor reserved for already-validated inputs; untrusted input \
                  must go through try_new"
    )]
    pub fn new(name: impl Into<String>) -> Self {
        // SAFETY: `new` is the infallible constructor reserved for inputs the caller
        // has already validated (compile-time literals, values that
        // round-tripped through `try_new` at a boundary). Untrusted input must
        // go through `try_new`.
        Self::try_new(name).expect("AgentName validation failed")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn system() -> Self {
        Self("system".to_owned())
    }
}

crate::__define_id_validated_conversions!(AgentName);
crate::__define_id_common!(AgentName);
