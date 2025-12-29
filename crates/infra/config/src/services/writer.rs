use super::types::{DeployEnvironment, EnvironmentConfig};
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use systemprompt_core_logging::CliService;

#[derive(Debug)]
pub struct ConfigWriter {
    project_root: PathBuf,
}

impl ConfigWriter {
    pub const fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }

    pub fn write_env_file(config: &EnvironmentConfig, output_path: &Path) -> Result<()> {
        let mut lines: Vec<String> = config
            .variables
            .iter()
            .map(|(k, v)| {
                if v.contains(char::is_whitespace) {
                    format!("{}=\"{}\"", k, v)
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
        CliService::info(&format!("   {} environment variables generated", var_count));

        Ok(())
    }

    pub fn write_web_env_file(&self, config: &EnvironmentConfig) -> Result<()> {
        let web_env_path = self
            .project_root
            .join("core/web")
            .join(format!(".env.{}", config.environment.as_str()));

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
            let web_dir = self.project_root.join("core/web");
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
                    "Created symlink: {} -> {}",
                    env_link.display(),
                    target
                ));
            }
        }

        if config.environment == DeployEnvironment::DockerDev {
            let vite_docker_path = self.project_root.join("core/web/.env.docker");
            fs::write(&vite_docker_path, vite_vars.join("\n"))?;
            CliService::success(&format!(
                "Web configuration also written to: {}",
                vite_docker_path.display()
            ));
        }

        Ok(())
    }
}
