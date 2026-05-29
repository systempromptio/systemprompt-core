//! `ConfigService` — generate environment-specific deployment configs
//! by merging `infrastructure/environments/<env>/config.yaml` over a
//! shared `base.yaml`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_logging::CliService;
use systemprompt_models::{contains_placeholder, interpolate, read_env_optional};

use super::types::{DeployEnvironment, EnvironmentConfig};
use super::writer::ConfigWriter;
use crate::error::{ConfigError, ConfigResult};

#[derive(Debug)]
pub struct ConfigService {
    project_root: PathBuf,
    environments_dir: PathBuf,
    writer: ConfigWriter,
}

impl ConfigService {
    #[must_use]
    pub fn new(project_root: PathBuf) -> Self {
        let environments_dir = project_root.join("infrastructure/environments");
        let writer = ConfigWriter::new(project_root.clone());
        Self {
            project_root,
            environments_dir,
            writer,
        }
    }

    pub fn generate_config(
        &self,
        environment: DeployEnvironment,
    ) -> ConfigResult<EnvironmentConfig> {
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
            return Err(ConfigError::EnvironmentConfigMissing {
                path: base_config_path,
            });
        }

        if !env_config_path.exists() {
            return Err(ConfigError::EnvironmentConfigMissing {
                path: env_config_path,
            });
        }

        let secrets = self.load_secrets()?;

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

        let resolved = Self::resolve_variables(merged, &secrets)?;

        CliService::success("   Configuration generated successfully");

        Ok(EnvironmentConfig {
            environment,
            variables: resolved,
        })
    }

    fn load_secrets(&self) -> ConfigResult<HashMap<String, String>> {
        let secrets_file = self.project_root.join(".env.secrets");
        let mut secrets = HashMap::new();

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
                    secrets.insert(
                        key.trim().to_owned(),
                        value.trim().trim_matches('"').to_owned(),
                    );
                }
            }

            CliService::success("   Secrets loaded");
        } else {
            CliService::warning("   No .env.secrets file found");
        }

        Ok(secrets)
    }

    fn yaml_to_flat_map(yaml_path: &Path) -> ConfigResult<HashMap<String, String>> {
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
            serde_yaml::Value::Sequence(_) => {
                tracing::warn!(key = %prefix, "YAML sequences are not supported in config flattening - skipping");
            },
            _ => {
                if let Some(str_val) = value.as_str() {
                    result.insert(prefix, str_val.to_owned());
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

    fn resolve_variables(
        mut vars: HashMap<String, String>,
        secrets: &HashMap<String, String>,
    ) -> ConfigResult<HashMap<String, String>> {
        const MAX_PASSES: usize = 5;

        for current_pass in 0..MAX_PASSES {
            let mut changes_made = false;

            for (key, value) in vars.clone() {
                let resolved = interpolate(&value, &|name| {
                    secrets
                        .get(name)
                        .cloned()
                        .or_else(|| read_env_optional(name))
                        .or_else(|| vars.get(name).cloned())
                });

                if resolved != value {
                    vars.insert(key, resolved);
                    changes_made = true;
                }
            }

            if !changes_made {
                break;
            }

            // Reaching the final pass while still mutating means a cycle or a
            // reference chain deeper than MAX_PASSES — surface it rather than
            // returning a config that still carries placeholders.
            if current_pass == MAX_PASSES - 1 {
                let unresolved: Vec<_> = vars
                    .iter()
                    .filter(|(_, v)| contains_placeholder(v))
                    .map(|(k, v)| format!("{k} = {v}"))
                    .collect();

                if !unresolved.is_empty() {
                    return Err(ConfigError::UnresolvedVariables {
                        passes: MAX_PASSES,
                        unresolved: unresolved.join("\n"),
                    });
                }
            }
        }

        Ok(vars)
    }

    pub fn write_env_file(config: &EnvironmentConfig, output_path: &Path) -> ConfigResult<()> {
        ConfigWriter::write_env_file(config, output_path)
    }

    pub fn write_web_env_file(&self, config: &EnvironmentConfig) -> ConfigResult<()> {
        self.writer.write_web_env_file(config)
    }
}
