use crate::McpServerConfig;
use crate::error::McpDomainResult;
use std::fs;
use std::path::Path;
use std::process::Command;
use systemprompt_config::{ProfileBootstrap, SecretsBootstrap};
use systemprompt_models::{AppPaths, Config, Secrets};

const MAX_LOG_SIZE: u64 = 10 * 1024 * 1024;

struct SpawnEnvironment<'a> {
    config: &'a McpServerConfig,
    paths: &'a AppPaths,
    config_global: &'a Config,
    secrets: &'a Secrets,
    profile_path: &'a str,
    tools_config_json: &'a str,
    server_model_config_json: &'a str,
}

fn configure_environment(command: &mut Command, env: &SpawnEnvironment<'_>) {
    let SpawnEnvironment {
        config,
        paths,
        config_global,
        secrets,
        profile_path,
        tools_config_json,
        server_model_config_json,
    } = env;

    command.env_clear();
    if let Ok(path) = std::env::var("PATH") {
        command.env("PATH", path);
    }
    if let Ok(home) = std::env::var("HOME") {
        command.env("HOME", home);
    }
    // SSRF guard allowlist (see systemprompt_models::net::TRUSTED_HTTP_HOSTS_ENV).
    // The MCP child re-validates outbound URLs when it loads the profile catalog,
    // so the operator's process-wide trust assertion must travel with it —
    // env_clear would otherwise leave the child running with an empty allowlist
    // and reject sealed-network hostnames the parent already accepted.
    if let Ok(trusted) = std::env::var(systemprompt_models::net::TRUSTED_HTTP_HOSTS_ENV) {
        command.env(systemprompt_models::net::TRUSTED_HTTP_HOSTS_ENV, trusted);
    }

    command
        .env("SYSTEMPROMPT_PROFILE", profile_path)
        .env("SYSTEMPROMPT_SUBPROCESS", "1")
        .env("DATABASE_TYPE", &config_global.database_type)
        .env("MCP_SERVICE_ID", &config.name)
        .env("MCP_PORT", config.port.to_string())
        .env("MCP_TOOLS_CONFIG", tools_config_json)
        .env("MCP_SERVER_MODEL_CONFIG", server_model_config_json)
        .env("SYSTEM_PATH", paths.system().root());

    for (k, v) in secrets.to_subprocess_env() {
        command.env(k, v);
    }

    for var_name in &config.env_vars {
        match std::env::var(var_name) {
            Ok(value) => {
                command.env(var_name, value);
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
}

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

pub fn spawn_server(paths: &AppPaths, config: &McpServerConfig) -> McpDomainResult<u32> {
    let binary_path = paths.build().resolve_binary(&config.binary).map_err(|e| {
        crate::error::McpDomainError::Internal(format!("{}: {e}", {
            format!(
                "Failed to find binary '{}' for {}",
                config.binary, config.name
            )
        }))
    })?;

    let config_global = Config::get()?;

    let log_dir = paths.system().logs();
    fs::create_dir_all(&log_dir).map_err(|e| {
        crate::error::McpDomainError::Internal(format!(
            "Failed to create logs directory: {}: {e}",
            log_dir.display()
        ))
    })?;

    let log_file_path = log_dir.join(format!("mcp-{}.log", config.name));
    rotate_log_if_needed(&log_file_path);

    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file_path)
        .map_err(|e| {
            crate::error::McpDomainError::Internal(format!(
                "Failed to create log file: {}: {e}",
                log_file_path.display()
            ))
        })?;

    let tools_config_json = serde_json::to_string(&config.tools).map_err(|e| {
        crate::error::McpDomainError::Internal(format!("Failed to serialize tools config: {e}"))
    })?;
    let server_model_config_json = serde_json::to_string(&config.model_config).map_err(|e| {
        crate::error::McpDomainError::Internal(format!(
            "Failed to serialize server model config: {e}"
        ))
    })?;

    let profile_path = ProfileBootstrap::get_path().map_err(|e| {
        crate::error::McpDomainError::Internal(format!(
            "SYSTEMPROMPT_PROFILE not set - cannot spawn MCP server: {e}"
        ))
    })?;
    let secrets = SecretsBootstrap::get().map_err(|e| {
        crate::error::McpDomainError::Internal(format!(
            "Secrets not available - cannot spawn MCP server: {e}"
        ))
    })?;

    let mut child_command = Command::new(&binary_path);
    configure_environment(
        &mut child_command,
        &SpawnEnvironment {
            config,
            paths,
            config_global,
            secrets,
            profile_path,
            tools_config_json: &tools_config_json,
            server_model_config_json: &server_model_config_json,
        },
    );

    let child = child_command
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::from(log_file))
        .stdin(std::process::Stdio::null())
        .spawn()
        .map_err(|e| {
            crate::error::McpDomainError::Internal(format!(
                "Failed to start detached {}: {e}",
                config.name
            ))
        })?;

    let pid = child.id();

    #[expect(
        clippy::mem_forget,
        reason = "detached MCP server: skip Child's drop-time wait so the OS keeps the process \
                  alive after this fn returns"
    )]
    std::mem::forget(child);

    Ok(pid)
}

pub fn verify_binary(paths: &AppPaths, config: &McpServerConfig) -> McpDomainResult<()> {
    let binary_path = paths.build().resolve_binary(&config.binary)?;

    let metadata = fs::metadata(&binary_path).map_err(|e| {
        crate::error::McpDomainError::Internal(format!(
            "Binary not found: {}: {e}",
            binary_path.display()
        ))
    })?;

    tracing::debug!(
        service = %config.name,
        binary = %config.binary,
        path = %binary_path.display(),
        size = metadata.len(),
        "Binary verified"
    );
    Ok(())
}

pub fn build_server(config: &McpServerConfig) -> McpDomainResult<()> {
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
        .map_err(|e| {
            crate::error::McpDomainError::Internal(format!("{}: {e}", {
                format!(
                    "Failed to build {} (binary: {})",
                    config.name, config.binary
                )
            }))
        })?;

    if output.status.success() {
        tracing::info!(service = %config.name, binary = %config.binary, "Build completed (debug)");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!(service = %config.name, binary = %config.binary, error = %stderr, "Build failed");
        Err(crate::error::McpDomainError::Internal(format!(
            "Build failed for {} (binary: {})",
            config.name, config.binary
        )))
    }
}
