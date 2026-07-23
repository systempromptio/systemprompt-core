//! Detached spawning and binary management for MCP server processes.
//!
//! [`spawn_server`] launches an MCP server binary in its own process group with
//! a sanitised environment (profile, secrets, per-server config, and the SSRF
//! trust allowlist), redirecting output to a size-rotated log file and
//! detaching the child so it outlives this call. Also covers binary
//! verification and an on-demand debug build path.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::McpServerConfig;
use crate::error::McpDomainResult;
use std::fs;
use std::path::Path;
use std::process::Command;
use systemprompt_config::{ProfileBootstrap, SecretsBootstrap};
use systemprompt_models::{AppPaths, Config, Secrets};

const MAX_LOG_SIZE: u64 = 10 * 1024 * 1024;

#[derive(Debug)]
pub struct SpawnEnvSpec<'a> {
    pub config: &'a McpServerConfig,
    pub system_root: &'a Path,
    pub database_type: &'a str,
    pub profile_path: &'a str,
    pub tools_config_json: &'a str,
    pub server_model_config_json: &'a str,
}

pub fn build_environment(
    spec: &SpawnEnvSpec<'_>,
    secrets_env: &[(String, String)],
    lookup: impl Fn(&str) -> Option<String>,
) -> Vec<(String, String)> {
    let mut env = Vec::new();

    for inherited in ["PATH", "HOME"] {
        if let Some(value) = lookup(inherited) {
            env.push((inherited.to_owned(), value));
        }
    }
    // Why: SSRF guard allowlist (see
    // systemprompt_models::net::TRUSTED_HTTP_HOSTS_ENV). The MCP child
    // re-validates outbound URLs when it loads the profile catalog,
    // so the operator's process-wide trust assertion must travel with it —
    // env_clear would otherwise leave the child running with an empty allowlist
    // and reject sealed-network hostnames the parent already accepted.
    if let Some(trusted) = lookup(systemprompt_models::net::TRUSTED_HTTP_HOSTS_ENV) {
        env.push((
            systemprompt_models::net::TRUSTED_HTTP_HOSTS_ENV.to_owned(),
            trusted,
        ));
    }

    env.push((
        "SYSTEMPROMPT_PROFILE".to_owned(),
        spec.profile_path.to_owned(),
    ));
    env.push((
        systemprompt_models::subprocess::SUBPROCESS_MARKER_ENV.to_owned(),
        "1".to_owned(),
    ));
    env.push(("DATABASE_TYPE".to_owned(), spec.database_type.to_owned()));
    env.push((
        systemprompt_models::subprocess::MCP_SERVICE_ID_ENV.to_owned(),
        spec.config.name.clone(),
    ));
    env.push(("MCP_PORT".to_owned(), spec.config.port.to_string()));
    env.push((
        "MCP_TOOLS_CONFIG".to_owned(),
        spec.tools_config_json.to_owned(),
    ));
    env.push((
        "MCP_SERVER_MODEL_CONFIG".to_owned(),
        spec.server_model_config_json.to_owned(),
    ));
    env.push((
        "SYSTEM_PATH".to_owned(),
        spec.system_root.display().to_string(),
    ));

    env.extend(secrets_env.iter().cloned());

    for var_name in &spec.config.env_vars {
        match lookup(var_name) {
            Some(value) => env.push((var_name.clone(), value)),
            None => {
                tracing::warn!(
                    var = %var_name,
                    service = %spec.config.name,
                    "Optional env var not set for MCP server"
                );
            },
        }
    }

    env
}

fn configure_environment(command: &mut Command, spec: &SpawnEnvSpec<'_>, secrets: &Secrets) {
    command.env_clear();
    for (key, value) in build_environment(spec, &secrets.to_subprocess_env(), |name| {
        std::env::var(name).ok()
    }) {
        command.env(key, value);
    }
}

pub fn rotate_log_if_needed(log_path: &Path) {
    if let Ok(metadata) = fs::metadata(log_path)
        && metadata.len() > MAX_LOG_SIZE
    {
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

pub fn open_server_log(paths: &AppPaths, config: &McpServerConfig) -> McpDomainResult<fs::File> {
    let log_dir = paths.system().logs();
    fs::create_dir_all(&log_dir).map_err(|e| {
        crate::error::McpDomainError::Internal(format!(
            "Failed to create logs directory: {}: {e}",
            log_dir.display()
        ))
    })?;

    let log_file_path = log_dir.join(format!("mcp-{}.log", config.name));
    rotate_log_if_needed(&log_file_path);

    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file_path)
        .map_err(|e| {
            crate::error::McpDomainError::Internal(format!(
                "Failed to create log file: {}: {e}",
                log_file_path.display()
            ))
        })
}

pub fn serialize_server_configs(config: &McpServerConfig) -> McpDomainResult<(String, String)> {
    let tools_config_json = serde_json::to_string(&config.tools).map_err(|e| {
        crate::error::McpDomainError::Internal(format!("Failed to serialize tools config: {e}"))
    })?;
    let server_model_config_json = serde_json::to_string(&config.model_config).map_err(|e| {
        crate::error::McpDomainError::Internal(format!(
            "Failed to serialize server model config: {e}"
        ))
    })?;
    Ok((tools_config_json, server_model_config_json))
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

    let log_file = open_server_log(paths, config)?;
    let (tools_config_json, server_model_config_json) = serialize_server_configs(config)?;

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
        &SpawnEnvSpec {
            config,
            system_root: paths.system().root(),
            database_type: &config_global.database_type,
            profile_path,
            tools_config_json: &tools_config_json,
            server_model_config_json: &server_model_config_json,
        },
        secrets,
    );

    child_command
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::from(log_file))
        .stdin(std::process::Stdio::null());
    place_in_own_process_group(&mut child_command);

    let child = child_command.spawn().map_err(|e| {
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

#[cfg(unix)]
fn place_in_own_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    // Why: pgid 0 makes the child its own group leader (pgid == pid), so the
    // supervisor can signal the whole group on shutdown rather than orphaning
    // any helper processes the server spawns.
    command.process_group(0);
}

#[cfg(windows)]
fn place_in_own_process_group(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    command.creation_flags(CREATE_NEW_PROCESS_GROUP);
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
