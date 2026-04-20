//! Cloud configuration.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CloudConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,

    #[serde(default)]
    pub validation: CloudValidationMode,
}

impl CloudConfig {
    #[must_use]
    pub fn is_local_trial(&self) -> bool {
        self.tenant_id
            .as_deref()
            .is_some_and(|t| t.starts_with("local_"))
            || matches!(
                self.validation,
                CloudValidationMode::Warn | CloudValidationMode::Skip
            )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CloudValidationMode {
    #[default]
    Strict,
    Warn,
    Skip,
}
