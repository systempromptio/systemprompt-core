use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SecretsSource {
    File,
    Env,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SecretsConfig {
    pub secrets_path: String,

    #[serde(default)]
    pub validation: SecretsValidationMode,

    pub source: SecretsSource,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum SecretsValidationMode {
    Strict,

    #[default]
    Warn,

    Skip,
}
