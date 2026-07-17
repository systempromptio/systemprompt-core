//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use std::process::Command;
use systemprompt_cloud::constants::docker::{COMPOSE_PATH, container_name};
use systemprompt_logging::CliService;

use super::SetupArgs;
use super::common::{PostgresConfig, enable_extensions, generate_password, test_connection};
use super::docker_compose::{
    create_compose_files_if_missing, is_compose_available, is_container_running,
    is_docker_available, start_compose, wait_for_postgres_ready,
};
use super::docker_database::create_database_in_docker;
use crate::interactive::Prompter;

pub(super) async fn setup_docker_postgres_non_interactive(
    config: &PostgresConfig,
    env_name: &str,
) -> Result<PostgresConfig> {
    if !is_docker_available() {
        anyhow::bail!("Docker is not installed or not in PATH.");
    }
    if !is_compose_available() {
        anyhow::bail!("Docker Compose is not available.");
    }

    let compose_dir = std::env::current_dir()?.join(COMPOSE_PATH);
    let container = container_name(env_name);
    create_compose_files_if_missing(&compose_dir, &container, config.port)?;

    if is_container_running(&container) {
        if !test_connection(config).await {
            create_database_in_docker(config, &container).await?;
        }
        enable_extensions(config).await?;
        return Ok(config.clone());
    }

    start_compose(config, &compose_dir, &container)?;
    wait_for_postgres_ready(config, &container);
    enable_extensions(config).await?;

    Ok(config.clone())
}

pub(super) async fn setup_docker_postgres_interactive(
    args: &SetupArgs,
    prompter: &dyn Prompter,
    env_name: &str,
) -> Result<PostgresConfig> {
    CliService::info("Setting up PostgreSQL with Docker...");

    if !is_docker_available() {
        anyhow::bail!(
            "Docker is not installed or not in PATH.\nInstall Docker: https://docs.docker.com/get-docker/"
        );
    }

    if !is_compose_available() {
        anyhow::bail!(
            "Docker Compose is not available.\nEnsure Docker Desktop is installed or install \
             docker-compose."
        );
    }

    CliService::success("Docker and Docker Compose are available");

    let user = prompter.input_with_default("Database user", &args.effective_db_user(env_name))?;

    let password = args.db_password.clone().unwrap_or_else(generate_password);
    CliService::success(&format!("Generated password: {}", password));

    let database =
        prompter.input_with_default("Database name", &args.effective_db_name(env_name))?;

    let port_input = prompter.input_with_default("PostgreSQL port", &args.db_port.to_string())?;
    let port: u16 = port_input
        .trim()
        .parse()
        .with_context(|| format!("Invalid PostgreSQL port: {}", port_input))?;

    let config = PostgresConfig {
        host: "localhost".to_owned(),
        port,
        user,
        password,
        database,
    };

    let compose_dir = std::env::current_dir()?.join(COMPOSE_PATH);
    let container = container_name(env_name);
    create_compose_files_if_missing(&compose_dir, &container, port)?;
    if is_container_running(&container) {
        CliService::info(&format!(
            "PostgreSQL container '{}' is already running",
            container
        ));

        let reuse = prompter.confirm("Use existing container?", true)?;

        if reuse {
            if !test_connection(&config).await {
                CliService::info("Creating database and user in existing container...");
                create_database_in_docker(&config, &container).await?;
            }
            enable_extensions(&config).await?;
            return Ok(config);
        }

        CliService::info("Stopping existing container...");
        if let Err(e) = Command::new("docker").args(["stop", &container]).output() {
            tracing::warn!(container = %container, error = %e, "docker stop failed");
        }
        if let Err(e) = Command::new("docker").args(["rm", &container]).output() {
            tracing::warn!(container = %container, error = %e, "docker rm failed");
        }
    }

    start_compose(&config, &compose_dir, &container)?;

    wait_for_postgres_ready(&config, &container);

    enable_extensions(&config).await?;

    Ok(config)
}
