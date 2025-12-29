//! System paths configuration.

use super::Config;

#[derive(Debug, Clone)]
pub struct PathNotConfiguredError {
    pub path_name: String,
    pub profile_path: Option<String>,
}

impl PathNotConfiguredError {
    pub fn new(path_name: impl Into<String>) -> Self {
        use crate::profile_bootstrap::ProfileBootstrap;
        Self {
            path_name: path_name.into(),
            profile_path: ProfileBootstrap::get_path().ok().map(ToString::to_string),
        }
    }
}

impl std::fmt::Display for PathNotConfiguredError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Profile Error: Required path not configured\n")?;
        writeln!(f, "  Field: paths.{}", self.path_name)?;
        if let Some(ref profile) = self.profile_path {
            writeln!(f, "  Profile: {}", profile)?;
        }
        writeln!(f, "\n  To fix:")?;
        writeln!(
            f,
            "  - Run 'systemprompt cloud config' to regenerate profile"
        )?;
        write!(
            f,
            "  - Or manually add paths.{} to your profile",
            self.path_name
        )
    }
}

impl std::error::Error for PathNotConfiguredError {}

#[derive(Debug, Copy, Clone)]
pub struct SystemPaths;

impl SystemPaths {
    const METADATA_MCP: &'static str = "metadata/mcp";
    const SKILL_FILE: &'static str = "SKILL.md";
    const AGENTS_CONFIG_FILE: &'static str = "agents.yaml";
    const CONFIG_FILE: &'static str = "config.yaml";

    pub fn metadata_mcp(config: &Config) -> std::path::PathBuf {
        std::path::Path::new(&config.system_path).join(Self::METADATA_MCP)
    }

    pub fn services(config: &Config) -> std::path::PathBuf {
        std::path::PathBuf::from(&config.services_path)
    }

    pub fn skills(config: &Config) -> std::path::PathBuf {
        std::path::PathBuf::from(&config.skills_path)
    }

    pub fn config_dir(config: &Config) -> std::path::PathBuf {
        let path = std::path::Path::new(&config.settings_path);
        path.parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| path.to_path_buf())
    }

    pub fn agents_config(config: &Config) -> std::path::PathBuf {
        std::path::PathBuf::from(&config.settings_path)
    }

    pub fn services_config(config: &Config) -> std::path::PathBuf {
        std::path::PathBuf::from(&config.settings_path)
    }

    pub const fn skill_file() -> &'static str {
        Self::SKILL_FILE
    }

    pub const fn agents_config_file() -> &'static str {
        Self::AGENTS_CONFIG_FILE
    }

    pub const fn config_file() -> &'static str {
        Self::CONFIG_FILE
    }

    pub fn resolve_mcp_server(config: &Config, server_name: &str) -> std::path::PathBuf {
        Self::services(config).join(server_name)
    }

    pub fn resolve_skill(config: &Config, skill_name: &str) -> std::path::PathBuf {
        Self::skills(config).join(skill_name)
    }

    pub fn content_config(config: &Config) -> std::path::PathBuf {
        std::path::PathBuf::from(&config.content_config_path)
    }

    pub fn web_path(config: &Config) -> std::path::PathBuf {
        std::path::PathBuf::from(&config.web_path)
    }

    pub fn web_config(config: &Config) -> std::path::PathBuf {
        std::path::PathBuf::from(&config.web_config_path)
    }

    pub fn web_metadata(config: &Config) -> std::path::PathBuf {
        std::path::PathBuf::from(&config.web_metadata_path)
    }
}
