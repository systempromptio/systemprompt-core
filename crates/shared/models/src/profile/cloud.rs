//! Cloud configuration.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::TenantId;

#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CloudConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<TenantId>,

    #[serde(default)]
    pub validation: CloudValidationMode,
}

impl CloudConfig {
    #[must_use]
    pub fn is_local_trial(&self) -> bool {
        self.tenant_id
            .as_ref()
            .is_some_and(|t| t.as_str().starts_with("local_"))
            || matches!(
                self.validation,
                CloudValidationMode::Warn | CloudValidationMode::Skip
            )
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum CloudValidationMode {
    #[default]
    Strict,
    Warn,
    Skip,
}
