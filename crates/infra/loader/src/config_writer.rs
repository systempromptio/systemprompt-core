//! Writes individual agent files and patches the top-level config to
//! drop their `includes:` entries.
//!
//! A create or an update is checked: the new agent file is put in place
//! atomically (a temporary sibling renamed over it), the caller's check runs
//! against the tree as written, and when the check refuses the previous bytes
//! are put back the same way (or the new file removed), so a refused change
//! leaves the tree byte-identical and the error is
//! [`ConfigWriteError::Rejected`]. The check needs the change on disk because
//! the services loader resolves includes and discovery from the tree.
//! Concurrent writers racing on the same agent file may overwrite each other;
//! the loader does not lock the on-disk config.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_identifiers::AgentName;
use systemprompt_manifest::services::AgentConfig;

use crate::error::{ConfigWriteError, ConfigWriteResult};

/// What a caller's check of a written change refuses with.
pub type CheckError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Clone, Copy)]
pub struct ConfigWriter;

#[derive(serde::Serialize, serde::Deserialize)]
struct AgentFileContent {
    agents: HashMap<String, AgentConfig>,
}

impl ConfigWriter {
    pub fn create_agent(
        agent: &AgentConfig,
        services_dir: &Path,
        check: impl FnOnce() -> Result<(), CheckError>,
    ) -> ConfigWriteResult<PathBuf> {
        let agents_dir = services_dir.join("agents");
        fs::create_dir_all(&agents_dir).map_err(|e| ConfigWriteError::Io {
            path: agents_dir.clone(),
            source: e,
        })?;

        let agent_file = agents_dir.join(format!("{}.yaml", agent.name));

        if agent_file.exists() {
            return Err(ConfigWriteError::AgentFileExists(agent_file));
        }

        Self::write_checked(&agent_file, agent, check)?;

        Ok(agent_file)
    }

    pub fn update_agent(
        name: &AgentName,
        agent: &AgentConfig,
        services_dir: &Path,
        check: impl FnOnce() -> Result<(), CheckError>,
    ) -> ConfigWriteResult<()> {
        let agent_file = Self::find_agent_file(name, services_dir)?
            .ok_or_else(|| ConfigWriteError::AgentNotFound(name.clone()))?;

        Self::write_checked(&agent_file, agent, check)
    }

    fn write_checked(
        path: &Path,
        agent: &AgentConfig,
        check: impl FnOnce() -> Result<(), CheckError>,
    ) -> ConfigWriteResult<()> {
        let previous = match fs::read(path) {
            Ok(bytes) => Some(bytes),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => None,
            Err(source) => {
                return Err(ConfigWriteError::Io {
                    path: path.to_path_buf(),
                    source,
                });
            },
        };
        Self::replace_atomically(path, Self::agent_file_content(agent)?.as_bytes())?;
        let Err(refusal) = check() else {
            return Ok(());
        };
        match previous {
            Some(bytes) => Self::replace_atomically(path, &bytes)?,
            None => fs::remove_file(path).map_err(|source| ConfigWriteError::Io {
                path: path.to_path_buf(),
                source,
            })?,
        }
        Err(ConfigWriteError::Rejected(refusal))
    }

    fn replace_atomically(path: &Path, bytes: &[u8]) -> ConfigWriteResult<()> {
        let staged = path.with_file_name(format!(
            ".{}.staged",
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        ));
        let io = |source| ConfigWriteError::Io {
            path: path.to_path_buf(),
            source,
        };
        fs::write(&staged, bytes).map_err(io)?;
        fs::rename(&staged, path).map_err(|source| {
            if let Err(error) = fs::remove_file(&staged) {
                tracing::debug!(path = %staged.display(), %error, "staged agent file not removed");
            }
            ConfigWriteError::Io {
                path: path.to_path_buf(),
                source,
            }
        })
    }

    pub fn delete_agent(name: &AgentName, services_dir: &Path) -> ConfigWriteResult<()> {
        let agent_file = Self::find_agent_file(name, services_dir)?
            .ok_or_else(|| ConfigWriteError::AgentNotFound(name.clone()))?;

        fs::remove_file(&agent_file).map_err(|e| ConfigWriteError::Io {
            path: agent_file.clone(),
            source: e,
        })?;

        let config_path = services_dir.join("config/config.yaml");
        let include_path = format!("../agents/{name}.yaml");
        Self::remove_include(&include_path, &config_path)
    }

    pub fn find_agent_file(
        name: &AgentName,
        services_dir: &Path,
    ) -> ConfigWriteResult<Option<PathBuf>> {
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

    fn file_contains_agent(path: &Path, agent_name: &AgentName) -> ConfigWriteResult<bool> {
        let content = fs::read_to_string(path).map_err(|e| ConfigWriteError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;

        let parsed: AgentFileContent = serde_yaml::from_str(&content)?;

        Ok(parsed.agents.contains_key(agent_name.as_str()))
    }

    fn agent_file_content(agent: &AgentConfig) -> ConfigWriteResult<String> {
        let mut agents = HashMap::new();
        agents.insert(agent.name.clone(), agent.clone());

        let yaml = serde_yaml::to_string(&AgentFileContent { agents })?;

        let header = format!(
            "# {} Configuration\n# {}\n\n",
            agent.card.display_name, agent.card.description
        );

        Ok(format!("{header}{yaml}"))
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
