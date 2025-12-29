use super::types::{DeployEnvironment, EnvironmentConfig};
use super::writer::ConfigWriter;
use anyhow::{anyhow, Result};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use systemprompt_core_logging::CliService;

#[derive(Debug)]
pub struct ConfigManager {
    project_root: PathBuf,
    environments_dir: PathBuf,
    writer: ConfigWriter,
}

impl ConfigManager {
    pub fn new(project_root: PathBuf) -> Self {
        let environments_dir = project_root.join("infrastructure/environments");
        let writer = ConfigWriter::new(project_root.clone());
        Self {
            project_root,
            environments_dir,
            writer,
        }
    }

    pub fn generate_config(&self, environment: DeployEnvironment) -> Result<EnvironmentConfig> {
        CliService::info(&format!(
            "Building configuration for environment: {}",
            environment.as_str()
        ));

        let base_config_path = self.environments_dir.join("base.yaml");
        let env_config_path = self
            .environments_dir
            .join(environment.as_str())
            .join("config.yaml");

        if !base_config_path.exists() {
            return Err(anyhow!(
                "Base config not found: {}",
                base_config_path.display()
            ));
        }

        if !env_config_path.exists() {
            return Err(anyhow!(
                "Environment config not found: {}",
                env_config_path.display()
            ));
        }

        self.load_secrets()?;

        CliService::success(&format!(
            "   Parsing base config: {}",
            base_config_path.display()
        ));
        let base_vars = Self::yaml_to_flat_map(&base_config_path)?;

        CliService::success(&format!(
            "   Parsing environment config: {}",
            env_config_path.display()
        ));
        let env_vars = Self::yaml_to_flat_map(&env_config_path)?;

        let merged = Self::merge_configs(base_vars, env_vars);

        let resolved = Self::resolve_variables(merged)?;

        CliService::success("   Configuration generated successfully");

        Ok(EnvironmentConfig {
            environment,
            variables: resolved,
        })
    }

    fn load_secrets(&self) -> Result<()> {
        let secrets_file = self.project_root.join(".env.secrets");

        if secrets_file.exists() {
            CliService::info(&format!(
                "   Loading secrets from: {}",
                secrets_file.display()
            ));
            let content = fs::read_to_string(&secrets_file)?;

            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }

                if let Some((key, value)) = line.split_once('=') {
                    std::env::set_var(key.trim(), value.trim().trim_matches('"'));
                }
            }

            CliService::success("   Secrets loaded");
        } else {
            CliService::warning("   No .env.secrets file found");
        }

        Ok(())
    }

    fn yaml_to_flat_map(yaml_path: &Path) -> Result<HashMap<String, String>> {
        let content = fs::read_to_string(yaml_path)?;
        let yaml: serde_yaml::Value = serde_yaml::from_str(&content)?;

        let mut flat_map = HashMap::new();
        Self::flatten_yaml(&yaml, String::new(), &mut flat_map);

        Ok(flat_map)
    }

    fn flatten_yaml(
        value: &serde_yaml::Value,
        prefix: String,
        result: &mut HashMap<String, String>,
    ) {
        match value {
            serde_yaml::Value::Mapping(map) => {
                for (k, v) in map {
                    if let Some(key_str) = k.as_str() {
                        let new_prefix = if prefix.is_empty() {
                            key_str.to_uppercase()
                        } else {
                            format!("{}_{}", prefix, key_str.to_uppercase())
                        };
                        Self::flatten_yaml(v, new_prefix, result);
                    }
                }
            },
            serde_yaml::Value::Sequence(_) => {},
            _ => {
                if let Some(str_val) = value.as_str() {
                    result.insert(prefix, str_val.to_string());
                } else if let Some(num_val) = value.as_i64() {
                    result.insert(prefix, num_val.to_string());
                } else if let Some(bool_val) = value.as_bool() {
                    result.insert(prefix, bool_val.to_string());
                } else if let Some(float_val) = value.as_f64() {
                    result.insert(prefix, float_val.to_string());
                }
            },
        }
    }

    fn merge_configs(
        base: HashMap<String, String>,
        env: HashMap<String, String>,
    ) -> HashMap<String, String> {
        let mut merged = base;
        for (k, v) in env {
            merged.insert(k, v);
        }
        merged
    }

    fn resolve_variables(mut vars: HashMap<String, String>) -> Result<HashMap<String, String>> {
        let var_regex = Regex::new(r"\$\{([^}:]+)(?::-(.*?))?\}")?;
        let max_passes = 5;

        for current_pass in 0..max_passes {
            let mut changes_made = false;

            for (key, value) in vars.clone() {
                if var_regex.is_match(&value) {
                    let resolved = Self::resolve_value(&value, &vars, &var_regex)?;

                    if resolved != value {
                        vars.insert(key, resolved);
                        changes_made = true;
                    }
                }
            }

            if !changes_made {
                break;
            }

            if current_pass == max_passes - 1 && changes_made {
                let unresolved: Vec<_> = vars
                    .iter()
                    .filter(|(_, v)| var_regex.is_match(v))
                    .map(|(k, v)| format!("{k} = {v}"))
                    .collect();

                if !unresolved.is_empty() {
                    return Err(anyhow!(
                        "Failed to resolve after {} passes:\n{}",
                        max_passes,
                        unresolved.join("\n")
                    ));
                }
            }
        }

        Ok(vars)
    }

    fn resolve_value(
        value: &str,
        vars: &HashMap<String, String>,
        var_regex: &Regex,
    ) -> Result<String> {
        let mut result = value.to_string();

        for cap in var_regex.captures_iter(value) {
            let full_match = cap
                .get(0)
                .ok_or_else(|| anyhow!("Regex capture group 0 missing"))?
                .as_str();
            let var_name = cap
                .get(1)
                .ok_or_else(|| anyhow!("Regex capture group 1 missing"))?
                .as_str();
            let default_value = cap.get(2).map(|m| m.as_str());

            let replacement = std::env::var(var_name).ok().unwrap_or_else(|| {
                vars.get(var_name).map_or_else(
                    || default_value.map_or_else(|| full_match.to_string(), ToString::to_string),
                    Clone::clone,
                )
            });

            result = result.replace(full_match, &replacement);
        }

        Ok(result)
    }

    pub fn write_env_file(config: &EnvironmentConfig, output_path: &Path) -> Result<()> {
        ConfigWriter::write_env_file(config, output_path)
    }

    pub fn write_web_env_file(&self, config: &EnvironmentConfig) -> Result<()> {
        self.writer.write_web_env_file(config)
    }
}
