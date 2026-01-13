use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BuildExtensionRow {
    pub name: String,
    pub build_type: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BuildOutput {
    pub extensions: Vec<BuildExtensionRow>,
    pub total: usize,
    pub successful: usize,
    pub release_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CoreBuildOutput {
    pub target: String,
    pub mode: String,
    pub status: String,
    pub duration_secs: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WebBuildOutput {
    pub target: String,
    pub mode: String,
    pub status: String,
    pub output_dir: String,
    pub duration_secs: Option<f64>,
}
