//! Module type definitions.

use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    #[serde(skip_deserializing, default = "generate_uuid")]
    pub uuid: String,
    pub name: String,
    pub version: String,
    pub display_name: String,
    pub description: Option<String>,
    pub weight: Option<i32>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    pub schemas: Option<Vec<ModuleSchema>>,
    pub seeds: Option<Vec<ModuleSeed>>,
    pub permissions: Option<Vec<ModulePermission>>,
    #[serde(default)]
    pub audience: Vec<String>,
    #[serde(skip_deserializing, default)]
    pub enabled: bool,
    #[serde(default)]
    pub api: Option<ApiConfig>,
    #[serde(skip)]
    pub path: PathBuf,
}

impl Module {
    pub fn parse(content: &str, module_path: PathBuf) -> Result<Self> {
        let mut module: Self = serde_yaml::from_str(content)?;
        module.path = module_path;
        Ok(module)
    }
}

fn generate_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub type ModuleDefinition = Module;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub enabled: bool,
    #[serde(default)]
    pub path_prefix: Option<String>,
    #[serde(default)]
    pub openapi_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleSchema {
    pub file: String,
    pub table: String,
    pub required_columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleSeed {
    pub file: String,
    pub table: String,
    pub check_column: String,
    pub check_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModulePermission {
    pub name: String,
    pub description: String,
    pub resource: String,
    pub action: String,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModuleType {
    Regular,
    Proxy,
}
