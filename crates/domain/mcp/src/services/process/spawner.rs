//! Detached spawning and binary management for MCP server processes.
//!
//! [`spawn_server`] launches the server's program (its extension binary, or
//! this executable re-run as `admin mcp serve --server <id>` for a hosted
//! server, see [`SpawnProgram`]) in its own process group with
//! a sanitised environment (profile, the database URLs, the deployment's
//! public verification keys, granted secrets, per-server config, and the SSRF
//! trust allowlist; never the pepper, seed, signing PEMs or licence secret),
//! redirecting output to a size-rotated log file and detaching the child so it
//! outlives this call. Both programs get the same environment and the same
//! [`ChildKind::Mcp`] spawn marker, so ownership checks treat them alike.
//! Also covers binary verification and an on-demand debug build path, both of
//! which have nothing to do for a hosted server.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::McpServerConfig;
use crate::error::McpDomainResult;
use crate::services::spawn_target::{SpawnProgram, SpawnTarget, hosted_serve_args};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use systemprompt_config::SecretsService;
use systemprompt_config::paths::{AppPaths, BuildPaths};
use systemprompt_loader::subprocess::{self, ChildKind, Spawner};
use systemprompt_manifest::Config;

const MAX_LOG_SIZE: u64 = 10 * 1024 * 1024;

#[derive(Debug)]
pub struct SpawnEnvSpec<'a> {
    pub config: &'a McpServerConfig,
    pub port: u16,
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

    env.extend(systemprompt_models::subprocess::inherited_parent_env(
        &lookup,
    ));

    env.push((
        "SYSTEMPROMPT_PROFILE".to_owned(),
        spec.profile_path.to_owned(),
    ));
    env.push(("DATABASE_TYPE".to_owned(), spec.database_type.to_owned()));
    env.push(("MCP_PORT".to_owned(), spec.port.to_string()));
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

pub fn spawn_environment(
    spec: &SpawnEnvSpec<'_>,
    global: &Config,
    secrets: &SecretsService,
    lookup: impl Fn(&str) -> Option<String>,
) -> McpDomainResult<Vec<(String, String)>> {
    let secrets_env = super::child_env::child_secrets(global, secrets, spec.config)?;
    Ok(build_environment(spec, &secrets_env, lookup))
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
        crate::error::McpDomainError::operation(
            format!("Failed to create logs directory: {}", log_dir.display()),
            e,
        )
    })?;

    let log_file_path = log_dir.join(format!("mcp-{}.log", config.name));
    rotate_log_if_needed(&log_file_path);

    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file_path)
        .map_err(|e| {
            crate::error::McpDomainError::operation(
                format!("Failed to create log file: {}", log_file_path.display()),
                e,
            )
        })
}

pub fn serialize_server_configs(config: &McpServerConfig) -> McpDomainResult<(String, String)> {
    let tools_config_json = serde_json::to_string(&config.tools).map_err(|e| {
        crate::error::McpDomainError::operation("Failed to serialize tools config", e)
    })?;
    let server_model_config_json = serde_json::to_string(&config.model_config).map_err(|e| {
        crate::error::McpDomainError::operation("Failed to serialize server model config", e)
    })?;
    Ok((tools_config_json, server_model_config_json))
}

pub fn spawn_server(
    spawner: &Spawner,
    global: &Config,
    secrets: &SecretsService,
    paths: &AppPaths,
    config: &McpServerConfig,
) -> McpDomainResult<u32> {
    let port = config.spawn_port()?;
    let (program_path, args) = spawn_command_line(paths, config)?;

    let log_file = open_server_log(paths, config)?;
    let (tools_config_json, server_model_config_json) = serialize_server_configs(config)?;

    let profile_path = secrets.profile().path().to_str().ok_or_else(|| {
        crate::error::McpDomainError::Configuration(
            "profile path is not valid UTF-8 - cannot spawn MCP server".to_owned(),
        )
    })?;
    let env = spawn_environment(
        &SpawnEnvSpec {
            config,
            port,
            system_root: paths.system().root(),
            database_type: &global.database_type,
            profile_path,
            tools_config_json: &tools_config_json,
            server_model_config_json: &server_model_config_json,
        },
        global,
        secrets,
        |name| std::env::var(name).ok(),
    )?;

    let mut child_command = Command::new(&program_path);
    child_command.args(args);
    child_command.env_clear();
    child_command.envs(env);
    subprocess::mark_child(&mut child_command, ChildKind::Mcp, &config.service_name());

    child_command
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::from(log_file))
        .stdin(std::process::Stdio::null());
    subprocess::place_in_own_process_group(&mut child_command);

    let pid = spawner.spawn_supervised(child_command).map_err(|e| {
        crate::error::McpDomainError::operation(
            format!("Failed to start detached {}", config.name),
            e,
        )
    })?;

    Ok(pid)
}

pub fn spawn_command_line(
    paths: &AppPaths,
    config: &McpServerConfig,
) -> McpDomainResult<(PathBuf, Vec<String>)> {
    match config.spawn_program()? {
        SpawnProgram::Binary(binary) => {
            let binary_path = paths.build().resolve_binary(binary).map_err(|e| {
                crate::error::McpDomainError::operation(
                    format!("Failed to find binary '{binary}' for {}", config.name),
                    e,
                )
            })?;
            Ok((binary_path, Vec::new()))
        },
        SpawnProgram::Hosted => {
            let self_path = BuildPaths::resolve_self().map_err(|e| {
                crate::error::McpDomainError::operation(
                    format!("Failed to resolve the running binary to host {}", config.name),
                    e,
                )
            })?;
            Ok((self_path, hosted_serve_args(config)))
        },
    }
}

pub fn verify_binary(paths: &AppPaths, config: &McpServerConfig) -> McpDomainResult<()> {
    let SpawnProgram::Binary(binary) = config.spawn_program()? else {
        tracing::debug!(service = %config.name, "Hosted by this binary; nothing to verify");
        return Ok(());
    };
    let binary_path = paths.build().resolve_binary(binary)?;

    let metadata = fs::metadata(&binary_path).map_err(|e| {
        crate::error::McpDomainError::operation(
            format!("Binary not found: {}", binary_path.display()),
            e,
        )
    })?;

    tracing::debug!(
        service = %config.name,
        binary,
        path = %binary_path.display(),
        size = metadata.len(),
        "Binary verified"
    );
    Ok(())
}

pub fn build_server(config: &McpServerConfig) -> McpDomainResult<()> {
    let SpawnProgram::Binary(binary) = config.spawn_program()? else {
        tracing::debug!(service = %config.name, "Hosted by this binary; nothing to build");
        return Ok(());
    };
    tracing::info!(service = %config.name, binary, "Building service (debug mode)");

    let output = Command::new("cargo")
        .args(["build", "--package", binary, "--bin", binary])
        .output()
        .map_err(|e| {
            crate::error::McpDomainError::operation(
                format!("Failed to build {} (binary: {binary})", config.name),
                e,
            )
        })?;

    if output.status.success() {
        tracing::info!(service = %config.name, binary, "Build completed (debug)");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!(service = %config.name, binary, error = %stderr, "Build failed");
        Err(crate::error::McpDomainError::BuildFailed {
            service: config.name.clone(),
            binary: binary.to_owned(),
        })
    }
}
