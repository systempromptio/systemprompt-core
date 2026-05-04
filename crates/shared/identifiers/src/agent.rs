//! Agent identity newtypes: opaque [`AgentId`] (UUID-backed) and validated
//! [`AgentName`] (non-empty, reserves `"unknown"`).

crate::define_id!(AgentId, generate, schema);

use crate::error::IdValidationError;

/// Human-readable agent name. Empty strings and the literal `"unknown"`
/// (case-insensitive) are rejected — `"unknown"` is reserved as a
/// registry-miss sentinel.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, schemars::JsonSchema)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(feature = "sqlx", sqlx(transparent))]
#[serde(transparent)]
pub struct AgentName(String);

impl AgentName {
    /// Validates and constructs an `AgentName`, rejecting empty strings and
    /// the reserved value `"unknown"`.
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

    /// Constructs an `AgentName`, panicking on validation failure.
    // Why: panicking convenience constructor for static call sites where the input is known-valid;
    // clippy's expect lint is suppressed because failure here is a programmer-bug invariant.
    #[allow(clippy::expect_used)]
    pub fn new(name: impl Into<String>) -> Self {
        Self::try_new(name).expect("AgentName validation failed")
    }

    /// Returns the inner string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the canonical `"system"` agent name.
    pub fn system() -> Self {
        Self("system".to_string())
    }
}

crate::__define_id_validated_conversions!(AgentName);
crate::__define_id_common!(AgentName);
