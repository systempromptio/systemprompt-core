use super::ProcessManager;
use crate::McpServerConfig;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;
use systemprompt_models::{AppPaths, ProfileBootstrap, SecretsBootstrap};

const MAX_LOG_SIZE: u64 = 10 * 1024 * 1024;

fn rotate_log_if_needed(log_path: &Path) {
    if let Ok(metadata) = fs::metadata(log_path) {
        if metadata.len() > MAX_LOG_SIZE {
            let backup_path = log_path.with_extension("log.old");
            if let Err(e) = fs::rename(log_path, &backup_path) {
                tracing::warn!(
                    error = %e,
                    log_path = %log_path.display(),
                    backup_path = %backup_path.display(),
                    "Failed to rotate MCP log file"
                );
            }
        }
    }
}

pub fn spawn_server(_manager: &ProcessManager, config: &McpServerConfig) -> Result<u32> {
    let paths = AppPaths::get().map_err(|e| anyhow::anyhow!("{}", e))?;

    let binary_path = paths
        .build()
        .resolve_binary(&config.binary)
        .with_context(|| {
            format!(
                "Failed to find binary '{}' for {}",
                config.binary, config.name
            )
        })?;

    let config_global = systemprompt_models::Config::get()?;

    let log_dir = paths.system().logs();
    fs::create_dir_all(&log_dir)
        .with_context(|| format!("Failed to create logs directory: {}", log_dir.display()))?;

    let log_file_path = log_dir.join(format!("mcp-{}.log", config.name));
    rotate_log_if_needed(&log_file_path);

    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file_path)
        .with_context(|| format!("Failed to create log file: {}", log_file_path.display()))?;

    let tools_config_json =
        serde_json::to_string(&config.tools).context("Failed to serialize tools config")?;
    let server_model_config_json = serde_json::to_string(&config.model_config)
        .context("Failed to serialize server model config")?;

    let profile_path = ProfileBootstrap::get_path()
        .context("SYSTEMPROMPT_PROFILE not set - cannot spawn MCP server")?;
    let secrets =
        SecretsBootstrap::get().context("Secrets not available - cannot spawn MCP server")?;

    let mut child_command = Command::new(&binary_path);

    child_command
        .env("SYSTEMPROMPT_PROFILE", profile_path)
        .env("JWT_SECRET", &secrets.jwt_secret)
        .env("DATABASE_URL", &secrets.database_url)
        .env("DATABASE_TYPE", &config_global.database_type)
        .env("MCP_SERVICE_ID", &config.name)
        .env("MCP_PORT", config.port.to_string())
        .env("MCP_TOOLS_CONFIG", &tools_config_json)
        .env("MCP_SERVER_MODEL_CONFIG", &server_model_config_json);

    if let Some(key) = &secrets.gemini {
        child_command.env("GEMINI_API_KEY", key);
    }
    if let Some(key) = &secrets.anthropic {
        child_command.env("ANTHROPIC_API_KEY", key);
    }
    if let Some(key) = &secrets.openai {
        child_command.env("OPENAI_API_KEY", key);
    }
    if let Some(key) = &secrets.github {
        child_command.env("GITHUB_TOKEN", key);
    }

    if !secrets.custom.is_empty() {
        let custom_keys: Vec<&str> = secrets.custom.keys().map(String::as_str).collect();
        child_command.env("SYSTEMPROMPT_CUSTOM_SECRETS", custom_keys.join(","));
        for (key, value) in &secrets.custom {
            child_command.env(key, value);
        }
    }

    for var_name in &config.env_vars {
        match std::env::var(var_name) {
            Ok(value) => {
                child_command.env(var_name, value);
            },
            Err(_) => {
                tracing::warn!(
                    var = %var_name,
                    service = %config.name,
                    "Optional env var not set for MCP server"
                );
            },
        }
    }

    let child = child_command
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::from(log_file))
        .stdin(std::process::Stdio::null())
        .spawn()
        .with_context(|| format!("Failed to start detached {}", config.name))?;

    let pid = child.id();

    std::mem::forget(child);

    Ok(pid)
}

pub fn verify_binary(config: &McpServerConfig) -> Result<()> {
    let paths = AppPaths::get().map_err(|e| anyhow::anyhow!("{}", e))?;
    let binary_path = paths.build().resolve_binary(&config.binary)?;

    let metadata = fs::metadata(&binary_path)
        .with_context(|| format!("Binary not found: {}", binary_path.display()))?;

    tracing::debug!(
        service = %config.name,
        binary = %config.binary,
        path = %binary_path.display(),
        size = metadata.len(),
        "Binary verified"
    );
    Ok(())
}

pub fn build_server(config: &McpServerConfig) -> Result<()> {
    tracing::info!(service = %config.name, binary = %config.binary, "Building service (debug mode)");

    let output = Command::new("cargo")
        .args([
            "build",
            "--package",
            &config.binary,
            "--bin",
            &config.binary,
        ])
        .output()
        .with_context(|| {
            format!(
                "Failed to build {} (binary: {})",
                config.name, config.binary
            )
        })?;

    if output.status.success() {
        tracing::info!(service = %config.name, binary = %config.binary, "Build completed (debug)");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!(service = %config.name, binary = %config.binary, error = %stderr, "Build failed");
        Err(anyhow::anyhow!(
            "Build failed for {} (binary: {})",
            config.name,
            config.binary
        ))
    }
}
