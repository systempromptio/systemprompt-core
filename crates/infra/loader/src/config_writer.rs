//! Writes individual agent files and patches the top-level config to
//! drop their `includes:` entries.
//!
//! A create or an update is checked before it is visible: the new bytes are
//! staged in a sibling file with an unpredictable name created exclusively,
//! the caller's check validates the tree with that staged file standing in
//! for the live one ([`StagedAgentFile`], read by
//! [`ConfigLoader::with_staged`](crate::ConfigLoader::with_staged)), and only
//! an accepted change is renamed over the live file. A refused change removes
//! the staged file and leaves the live file byte-identical
//! ([`ConfigWriteError::Rejected`]); a staged file that cannot be removed is
//! [`ConfigWriteError::RejectedStagedNotRemoved`], carrying both causes.
//! Concurrent writers racing on the same agent file may overwrite each other;
//! the loader does not lock the on-disk config.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use systemprompt_identifiers::AgentName;
use systemprompt_manifest::services::AgentConfig;

use crate::error::{ConfigWriteError, ConfigWriteResult};

/// What a caller's check of a written change refuses with.
pub type CheckError = Box<dyn std::error::Error + Send + Sync>;

/// An agent file's new bytes, staged beside the live file and not yet visible.
#[derive(Debug, Clone, Copy)]
pub struct StagedAgentFile<'a> {
    live: &'a Path,
    staged: &'a Path,
    contents: &'a [u8],
}

impl<'a> StagedAgentFile<'a> {
    #[must_use]
    pub const fn live_path(&self) -> &'a Path {
        self.live
    }

    #[must_use]
    pub const fn staged_path(&self) -> &'a Path {
        self.staged
    }

    #[must_use]
    pub const fn contents(&self) -> &'a [u8] {
        self.contents
    }
}

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
        check: impl FnOnce(&StagedAgentFile<'_>) -> Result<(), CheckError>,
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
        check: impl FnOnce(&StagedAgentFile<'_>) -> Result<(), CheckError>,
    ) -> ConfigWriteResult<()> {
        let agent_file = Self::find_agent_file(name, services_dir)?
            .ok_or_else(|| ConfigWriteError::AgentNotFound(name.clone()))?;

        Self::write_checked(&agent_file, agent, check)
    }

    fn write_checked(
        path: &Path,
        agent: &AgentConfig,
        check: impl FnOnce(&StagedAgentFile<'_>) -> Result<(), CheckError>,
    ) -> ConfigWriteResult<()> {
        let io = |source| ConfigWriteError::Io {
            path: path.to_path_buf(),
            source,
        };
        let contents = Self::agent_file_content(agent)?;
        let dir = path.parent().unwrap_or_else(|| Path::new("."));
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut staged = tempfile::Builder::new()
            .prefix(&format!(".{file_name}."))
            .suffix(".staged")
            .tempfile_in(dir)
            .map_err(io)?;
        staged.write_all(contents.as_bytes()).map_err(io)?;
        staged.as_file().sync_all().map_err(io)?;

        let verdict = check(&StagedAgentFile {
            live: path,
            staged: staged.path(),
            contents: contents.as_bytes(),
        });
        match verdict {
            Ok(()) => staged
                .persist(path)
                .map(drop)
                .map_err(|failure| io(failure.error)),
            Err(refusal) => {
                let staged_path = staged.path().to_path_buf();
                match staged.close() {
                    Ok(()) => Err(ConfigWriteError::Rejected(refusal)),
                    Err(cleanup) => Err(ConfigWriteError::RejectedStagedNotRemoved {
                        staged: staged_path,
                        refusal,
                        cleanup,
                    }),
                }
            },
        }
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
