crate::define_id!(AgentId, generate, schema);

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

    #[allow(clippy::expect_used)]
    pub fn new(name: impl Into<String>) -> Self {
        Self::try_new(name).expect("AgentName validation failed")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn system() -> Self {
        Self("system".to_string())
    }
}

crate::__define_id_validated_conversions!(AgentName);
crate::__define_id_common!(AgentName);
