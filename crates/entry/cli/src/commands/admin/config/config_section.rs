use std::path::PathBuf;

use anyhow::{Context, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_models::ProfileBootstrap;

use super::rate_limit_types::ResetChange;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidateOutput {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExportOutput {
    pub format: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImportOutput {
    pub path: String,
    pub changes: Vec<ResetChange>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DiffOutput {
    pub source: String,
    pub differences: Vec<DiffEntry>,
    pub identical: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DiffEntry {
    pub field: String,
    pub current: String,
    pub other: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConfigFileInfo {
    pub path: String,
    pub section: String,
    pub exists: bool,
    pub valid: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConfigListOutput {
    pub total: usize,
    pub valid: usize,
    pub invalid: usize,
    pub files: Vec<ConfigFileInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConfigValidateOutput {
    pub files: Vec<ConfigFileInfo>,
    pub all_valid: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigSection {
    Ai,
    Content,
    Web,
    Scheduler,
    Agents,
    Mcp,
    Skills,
    Profile,
    Services,
}

impl ConfigSection {
    pub const fn all() -> &'static [Self] {
        &[
            Self::Profile,
            Self::Services,
            Self::Ai,
            Self::Content,
            Self::Web,
            Self::Scheduler,
            Self::Agents,
            Self::Mcp,
            Self::Skills,
        ]
    }

    pub fn file_path(self) -> Result<PathBuf> {
        let profile = ProfileBootstrap::get()?;
        match self {
            Self::Ai => Ok(PathBuf::from(&profile.paths.services).join("ai/config.yaml")),
            Self::Content => Ok(PathBuf::from(&profile.paths.services).join("content/config.yaml")),
            Self::Web => Ok(PathBuf::from(&profile.paths.services).join("web/config.yaml")),
            Self::Scheduler => {
                Ok(PathBuf::from(&profile.paths.services).join("scheduler/config.yaml"))
            },
            Self::Agents => Ok(PathBuf::from(&profile.paths.services).join("agents/config.yaml")),
            Self::Mcp => Ok(PathBuf::from(&profile.paths.services).join("mcp/config.yaml")),
            Self::Skills => Ok(PathBuf::from(&profile.paths.services).join("skills/config.yaml")),
            Self::Profile => Ok(PathBuf::from(ProfileBootstrap::get_path()?)),
            Self::Services => Ok(PathBuf::from(&profile.paths.services).join("config/config.yaml")),
        }
    }

    pub fn all_files(self) -> Result<Vec<PathBuf>> {
        let profile = ProfileBootstrap::get()?;
        let services_path = PathBuf::from(&profile.paths.services);

        match self {
            Self::Profile => Ok(vec![PathBuf::from(ProfileBootstrap::get_path()?)]),
            Self::Services => Ok(vec![services_path.join("config/config.yaml")]),
            Self::Ai => Self::collect_yaml_files(&services_path.join("ai")),
            Self::Content => Self::collect_yaml_files(&services_path.join("content")),
            Self::Web => Self::collect_yaml_files(&services_path.join("web")),
            Self::Scheduler => Self::collect_yaml_files(&services_path.join("scheduler")),
            Self::Agents => Self::collect_yaml_files(&services_path.join("agents")),
            Self::Mcp => Self::collect_yaml_files(&services_path.join("mcp")),
            Self::Skills => Self::collect_yaml_files(&services_path.join("skills")),
        }
    }

    fn collect_yaml_files(dir: &PathBuf) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        if dir.exists() {
            Self::collect_yaml_recursive(dir, &mut files)?;
        }
        Ok(files)
    }

    fn collect_yaml_recursive(dir: &PathBuf, files: &mut Vec<PathBuf>) -> Result<()> {
        if !dir.is_dir() {
            return Ok(());
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                Self::collect_yaml_recursive(&path, files)?;
            } else if let Some(ext) = path.extension() {
                if ext == "yaml" || ext == "yml" {
                    files.push(path);
                }
            }
        }
        Ok(())
    }
}

impl std::fmt::Display for ConfigSection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ai => write!(f, "ai"),
            Self::Content => write!(f, "content"),
            Self::Web => write!(f, "web"),
            Self::Scheduler => write!(f, "scheduler"),
            Self::Agents => write!(f, "agents"),
            Self::Mcp => write!(f, "mcp"),
            Self::Skills => write!(f, "skills"),
            Self::Profile => write!(f, "profile"),
            Self::Services => write!(f, "services"),
        }
    }
}

impl std::str::FromStr for ConfigSection {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "ai" => Ok(Self::Ai),
            "content" => Ok(Self::Content),
            "web" => Ok(Self::Web),
            "scheduler" => Ok(Self::Scheduler),
            "agents" => Ok(Self::Agents),
            "mcp" => Ok(Self::Mcp),
            "skills" => Ok(Self::Skills),
            "profile" => Ok(Self::Profile),
            "services" => Ok(Self::Services),
            _ => Err(anyhow::anyhow!("Unknown config section: {}", s)),
        }
    }
}

pub fn read_yaml_file(path: &std::path::Path) -> Result<serde_yaml::Value> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;
    serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse YAML from: {}", path.display()))
}

pub fn write_yaml_file(path: &std::path::Path, content: &serde_yaml::Value) -> Result<()> {
    let yaml_str = serde_yaml::to_string(content).with_context(|| "Failed to serialize YAML")?;
    std::fs::write(path, yaml_str)
        .with_context(|| format!("Failed to write file: {}", path.display()))
}
