use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExportOutput {
    pub exported_count: i64,
    pub output_directory: String,
    pub files: Vec<String>,
}
