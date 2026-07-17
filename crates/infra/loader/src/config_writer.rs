//! Writes individual agent files and patches the top-level config to
//! drop their `includes:` entries.
//!
//! All operations are atomic at the per-file level; concurrent writers
//! racing on the same agent file may overwrite each other and the loader
//! does not attempt to lock the on-disk config.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_models::services::AgentConfig;

use crate::error::{ConfigWriteError, ConfigWriteResult};

#[derive(Debug, Clone, Copy)]
pub struct ConfigWriter;

#[derive(serde::Serialize, serde::Deserialize)]
struct AgentFileContent {
    agents: HashMap<String, AgentConfig>,
}

impl ConfigWriter {
    pub fn create_agent(agent: &AgentConfig, services_dir: &Path) -> ConfigWriteResult<PathBuf> {
        let agents_dir = services_dir.join("agents");
        fs::create_dir_all(&agents_dir).map_err(|e| ConfigWriteError::Io {
            path: agents_dir.clone(),
            source: e,
        })?;

        let agent_file = agents_dir.join(format!("{}.yaml", agent.name));

        if agent_file.exists() {
            return Err(ConfigWriteError::AgentFileExists(agent_file));
        }

        Self::write_agent_file(&agent_file, agent)?;

        Ok(agent_file)
    }

    pub fn update_agent(
        name: &str,
        agent: &AgentConfig,
        services_dir: &Path,
    ) -> ConfigWriteResult<()> {
        let agent_file = Self::find_agent_file(name, services_dir)?
            .ok_or_else(|| ConfigWriteError::AgentNotFound(name.to_owned()))?;

        Self::write_agent_file(&agent_file, agent)
    }

    pub fn delete_agent(name: &str, services_dir: &Path) -> ConfigWriteResult<()> {
        let agent_file = Self::find_agent_file(name, services_dir)?
            .ok_or_else(|| ConfigWriteError::AgentNotFound(name.to_owned()))?;

        fs::remove_file(&agent_file).map_err(|e| ConfigWriteError::Io {
            path: agent_file.clone(),
            source: e,
        })?;

        let config_path = services_dir.join("config/config.yaml");
        let include_path = format!("../agents/{name}.yaml");
        Self::remove_include(&include_path, &config_path)
    }

    pub fn find_agent_file(name: &str, services_dir: &Path) -> ConfigWriteResult<Option<PathBuf>> {
        let agents_dir = services_dir.join("agents");

        if !agents_dir.exists() {
            return Ok(None);
        }

        let expected_file = agents_dir.join(format!("{name}.yaml"));
        if expected_file.exists() && Self::file_contains_agent(&expected_file, name)? {
            return Ok(Some(expected_file));
        }

        for entry in fs::read_dir(&agents_dir).map_err(|e| ConfigWriteError::Io {
            path: agents_dir.clone(),
            source: e,
        })? {
            let path = entry
                .map_err(|e| ConfigWriteError::Io {
                    path: agents_dir.clone(),
                    source: e,
                })?
                .path();

            if path
                .extension()
                .is_some_and(|ext| ext == "yaml" || ext == "yml")
                && Self::file_contains_agent(&path, name)?
            {
                return Ok(Some(path));
            }
        }

        Ok(None)
    }

    fn file_contains_agent(path: &Path, agent_name: &str) -> ConfigWriteResult<bool> {
        let content = fs::read_to_string(path).map_err(|e| ConfigWriteError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;

        let parsed: AgentFileContent = serde_yaml::from_str(&content)?;

        Ok(parsed.agents.contains_key(agent_name))
    }

    fn write_agent_file(path: &Path, agent: &AgentConfig) -> ConfigWriteResult<()> {
        let mut agents = HashMap::new();
        agents.insert(agent.name.clone(), agent.clone());

        let content = AgentFileContent { agents };

        let yaml = serde_yaml::to_string(&content)?;

        let header = format!(
            "# {} Configuration\n# {}\n\n",
            agent.card.display_name, agent.card.description
        );

        fs::write(path, format!("{header}{yaml}")).map_err(|e| ConfigWriteError::Io {
            path: path.to_path_buf(),
            source: e,
        })
    }

    fn remove_include(include_path: &str, config_path: &Path) -> ConfigWriteResult<()> {
        let content = fs::read_to_string(config_path).map_err(|e| ConfigWriteError::Io {
            path: config_path.to_path_buf(),
            source: e,
        })?;

        let search_pattern = format!("  - {include_path}");
        let quoted_pattern = format!("  - \"{include_path}\"");

        let new_lines: Vec<&str> = content
            .lines()
            .filter(|line| *line != search_pattern && *line != quoted_pattern)
            .collect();

        fs::write(config_path, new_lines.join("\n")).map_err(|e| ConfigWriteError::Io {
            path: config_path.to_path_buf(),
            source: e,
        })
    }
}
