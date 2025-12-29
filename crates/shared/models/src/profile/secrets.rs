use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SecretsSource {
    File,
    Env,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretsConfig {
    pub secrets_path: String,

    #[serde(default)]
    pub validation: SecretsValidationMode,

    pub source: SecretsSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SecretsValidationMode {
    Strict,

    #[default]
    Warn,

    Skip,
}
