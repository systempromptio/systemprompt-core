use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtensionValidationOutput {
    pub valid: bool,
    pub extension_count: usize,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidationError {
    pub extension_id: Option<String>,
    pub error_type: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidationWarning {
    pub extension_id: Option<String>,
    pub warning_type: String,
    pub message: String,
}
