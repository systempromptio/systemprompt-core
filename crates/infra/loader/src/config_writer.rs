use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_models::services::AgentConfig;

#[derive(Debug, Clone, Copy)]
pub struct ConfigWriter;

#[derive(serde::Serialize, serde::Deserialize)]
struct AgentFileContent {
    agents: HashMap<String, AgentConfig>,
}

impl ConfigWriter {
    pub fn create_agent(agent: &AgentConfig, services_dir: &Path) -> Result<PathBuf> {
        let agents_dir = services_dir.join("agents");
        fs::create_dir_all(&agents_dir)
            .with_context(|| format!("Failed to create agents directory: {}", agents_dir.display()))?;

        let agent_file = agents_dir.join(format!("{}.yaml", agent.name));

        if agent_file.exists() {
            return Err(anyhow!(
                "Agent file already exists: {}. Use 'agents edit' to modify.",
                agent_file.display()
            ));
        }

        Self::write_agent_file(&agent_file, agent)?;

        let config_path = services_dir.join("config/config.yaml");
        let include_path = format!("../agents/{}.yaml", agent.name);
        Self::add_include(&include_path, &config_path)?;

        Ok(agent_file)
    }

    pub fn update_agent(name: &str, agent: &AgentConfig, services_dir: &Path) -> Result<()> {
        let agent_file = Self::find_agent_file(name, services_dir)?
            .ok_or_else(|| anyhow!("Agent '{}' not found in any configuration file", name))?;

        Self::write_agent_file(&agent_file, agent)
    }

    pub fn delete_agent(name: &str, services_dir: &Path) -> Result<()> {
        let agent_file = Self::find_agent_file(name, services_dir)?
            .ok_or_else(|| anyhow!("Agent '{}' not found in any configuration file", name))?;

        fs::remove_file(&agent_file)
            .with_context(|| format!("Failed to delete agent file: {}", agent_file.display()))?;

        let config_path = services_dir.join("config/config.yaml");
        let include_path = format!("../agents/{}.yaml", name);
        Self::remove_include(&include_path, &config_path)
    }

    pub fn find_agent_file(name: &str, services_dir: &Path) -> Result<Option<PathBuf>> {
        let agents_dir = services_dir.join("agents");

        if !agents_dir.exists() {
            return Ok(None);
        }

        let expected_file = agents_dir.join(format!("{}.yaml", name));
        if expected_file.exists() && Self::file_contains_agent(&expected_file, name)? {
            return Ok(Some(expected_file));
        }

        for entry in fs::read_dir(&agents_dir)
            .with_context(|| format!("Failed to read agents directory: {}", agents_dir.display()))?
        {
            let path = entry?.path();

            if path.extension().is_some_and(|ext| ext == "yaml" || ext == "yml")
                && Self::file_contains_agent(&path, name)?
            {
                return Ok(Some(path));
            }
        }

        Ok(None)
    }

    fn file_contains_agent(path: &Path, agent_name: &str) -> Result<bool> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        let parsed: AgentFileContent = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse YAML file: {}", path.display()))?;

        Ok(parsed.agents.contains_key(agent_name))
    }

    fn write_agent_file(path: &Path, agent: &AgentConfig) -> Result<()> {
        let mut agents = HashMap::new();
        agents.insert(agent.name.clone(), agent.clone());

        let content = AgentFileContent { agents };

        let yaml = serde_yaml::to_string(&content)
            .context("Failed to serialize agent to YAML")?;

        let header = format!(
            "# {} Configuration\n# {}\n\n",
            agent.card.display_name,
            agent.card.description
        );

        fs::write(path, format!("{}{}", header, yaml))
            .with_context(|| format!("Failed to write agent file: {}", path.display()))
    }

    fn add_include(include_path: &str, config_path: &Path) -> Result<()> {
        let content = fs::read_to_string(config_path)
            .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;

        if content.contains(include_path) {
            return Ok(());
        }

        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            if line.starts_with("includes:") {
                let insert_pos = lines
                    .iter()
                    .enumerate()
                    .skip(i + 1)
                    .take_while(|(_, l)| l.starts_with("  - ") || l.trim().is_empty())
                    .filter(|(_, l)| l.starts_with("  - "))
                    .last()
                    .map_or(i + 1, |(idx, _)| idx + 1);

                let new_line = format!("  - {}", include_path);
                let new_lines: Vec<&str> = lines[..insert_pos]
                    .iter()
                    .copied()
                    .chain(std::iter::once(new_line.as_str()))
                    .chain(lines[insert_pos..].iter().copied())
                    .collect();

                return fs::write(config_path, new_lines.join("\n"))
                    .with_context(|| format!("Failed to write config file: {}", config_path.display()));
            }
        }

        fs::write(config_path, format!("includes:\n  - {}\n\n{}", include_path, content))
            .with_context(|| format!("Failed to write config file: {}", config_path.display()))
    }

    fn remove_include(include_path: &str, config_path: &Path) -> Result<()> {
        let content = fs::read_to_string(config_path)
            .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;

        let search_pattern = format!("  - {}", include_path);
        let quoted_pattern = format!("  - \"{}\"", include_path);

        let new_lines: Vec<&str> = content
            .lines()
            .filter(|line| *line != search_pattern && *line != quoted_pattern)
            .collect();

        fs::write(config_path, new_lines.join("\n"))
            .with_context(|| format!("Failed to write config file: {}", config_path.display()))
    }
}
