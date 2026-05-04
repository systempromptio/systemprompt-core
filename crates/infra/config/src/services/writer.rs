//! On-disk writers for the deployment `.env` files produced by the
//! [`super::ConfigManager`] pipeline.

use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_logging::CliService;

use super::types::{DeployEnvironment, EnvironmentConfig};
use crate::error::ConfigResult;

/// Writes `.env`, `.env.<env>`, and the corresponding `web/` files.
#[derive(Debug)]
pub struct ConfigWriter {
    project_root: PathBuf,
}

impl ConfigWriter {
    /// Create a writer rooted at `project_root`.
    #[must_use]
    pub const fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }

    fn resolve_web_dir(&self) -> PathBuf {
        let core_web = self.project_root.join("core/web");
        if core_web.exists() {
            return core_web;
        }
        self.project_root.join("web")
    }

    /// Write the resolved variables in `config` as a sorted `KEY=VAL`
    /// list to `output_path`.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::ConfigError::Io`] on any underlying
    /// write failure.
    pub fn write_env_file(config: &EnvironmentConfig, output_path: &Path) -> ConfigResult<()> {
        let mut lines: Vec<String> = config
            .variables
            .iter()
            .map(|(k, v)| {
                if v.contains(char::is_whitespace) {
                    format!("{k}=\"{v}\"")
                } else {
                    format!("{k}={v}")
                }
            })
            .collect();

        lines.sort();

        fs::write(output_path, lines.join("\n"))?;

        CliService::success(&format!(
            "Configuration written to: {}",
            output_path.display()
        ));

        let var_count = lines.len();
        CliService::info(&format!("   {var_count} environment variables generated"));

        Ok(())
    }

    /// Write the `VITE_*` subset of `config` to the web frontend
    /// directory, plus the `Local`/`DockerDev` symlink and Docker
    /// override.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::ConfigError::Io`] on any underlying
    /// write failure.
    pub fn write_web_env_file(&self, config: &EnvironmentConfig) -> ConfigResult<()> {
        let web_dir = self.resolve_web_dir();
        let web_env_path = web_dir.join(format!(".env.{}", config.environment.as_str()));

        let vite_vars: Vec<String> = config
            .variables
            .iter()
            .filter(|(k, _)| k.starts_with("VITE_"))
            .map(|(k, v)| format!("{k}={v}"))
            .collect();

        if vite_vars.is_empty() {
            CliService::warning("No VITE_* variables found in configuration");
            return Ok(());
        }

        fs::write(&web_env_path, vite_vars.join("\n"))?;
        CliService::success(&format!(
            "Web configuration written to: {}",
            web_env_path.display()
        ));

        if config.environment == DeployEnvironment::Local {
            let env_link = web_dir.join(".env");
            let target = ".env.local";

            #[cfg(unix)]
            {
                use std::os::unix::fs as unix_fs;
                if env_link.exists() {
                    fs::remove_file(&env_link)?;
                }
                unix_fs::symlink(target, &env_link)?;
                CliService::success(&format!(
                    "Created symlink: {} -> {target}",
                    env_link.display()
                ));
            }
        }

        if config.environment == DeployEnvironment::DockerDev {
            let vite_docker_path = web_dir.join(".env.docker");
            fs::write(&vite_docker_path, vite_vars.join("\n"))?;
            CliService::success(&format!(
                "Web configuration also written to: {}",
                vite_docker_path.display()
            ));
        }

        Ok(())
    }
}
