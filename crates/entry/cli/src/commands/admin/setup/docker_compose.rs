//! docker-compose file generation for the local database.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;
use systemprompt_logging::CliService;

use super::common::PostgresConfig;

pub fn is_docker_available() -> bool {
    Command::new("docker").arg("--version").output().is_ok()
}

pub fn is_compose_available() -> bool {
    Command::new("docker")
        .args(["compose", "version"])
        .output()
        .is_ok_and(|o| o.status.success())
}

pub fn is_container_running(container_name: &str) -> bool {
    Command::new("docker")
        .args([
            "ps",
            "--filter",
            &format!("name=^{}$", container_name),
            "--format",
            "{{.Names}}",
        ])
        .output()
        .is_ok_and(|o| !String::from_utf8_lossy(&o.stdout).trim().is_empty())
}

pub fn create_compose_files_if_missing(
    compose_dir: &Path,
    container_name: &str,
    port: u16,
) -> Result<()> {
    const INIT_SCRIPT: &str = r#"CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";
"#;

    let compose_file = compose_dir.join("docker-compose.yaml");

    std::fs::create_dir_all(compose_dir).context("Failed to create infrastructure/docker")?;

    let init_scripts_dir = compose_dir.join("init-scripts");
    std::fs::create_dir_all(&init_scripts_dir).context("Failed to create init-scripts")?;

    let compose_content = format!(
        r#"services:
  postgres:
    image: postgres:18-alpine
    container_name: {container_name}
    environment:
      POSTGRES_USER: ${{POSTGRES_USER:-systemprompt}}
      POSTGRES_PASSWORD: ${{POSTGRES_PASSWORD}}
      POSTGRES_DB: ${{POSTGRES_DB:-systemprompt}}
    ports:
      - "{port}:5432"
    volumes:
      - {container_name}_data:/var/lib/postgresql
      - ./init-scripts:/docker-entrypoint-initdb.d
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U ${{POSTGRES_USER:-systemprompt}}"]
      interval: 5s
      timeout: 5s
      retries: 5
    networks:
      - {container_name}_network

volumes:
  {container_name}_data:

networks:
  {container_name}_network:
    driver: bridge
"#
    );

    std::fs::write(&compose_file, compose_content)
        .context("Failed to write docker-compose.yaml")?;

    std::fs::write(init_scripts_dir.join("01-extensions.sql"), INIT_SCRIPT)
        .context("Failed to write init script")?;

    CliService::success(&format!("Created {}", compose_file.display()));

    Ok(())
}

pub fn start_compose(
    config: &PostgresConfig,
    compose_dir: &Path,
    container_name: &str,
) -> Result<()> {
    CliService::info("Starting PostgreSQL...");

    let result = Command::new("docker")
        .args(["compose", "up", "-d"])
        .current_dir(compose_dir)
        .env("POSTGRES_USER", &config.user)
        .env("POSTGRES_PASSWORD", &config.password)
        .env("POSTGRES_DB", &config.database)
        .env("CONTAINER_NAME", container_name)
        .output()
        .context("Failed to run docker compose")?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        anyhow::bail!("Failed to start PostgreSQL: {}", stderr);
    }

    CliService::success("PostgreSQL container started");
    Ok(())
}

pub fn wait_for_postgres_ready(config: &PostgresConfig, container_name: &str) {
    CliService::info("Waiting for PostgreSQL to be ready...");

    for _ in 0..30 {
        std::thread::sleep(std::time::Duration::from_secs(1));

        let health = Command::new("docker")
            .args([
                "exec",
                container_name,
                "pg_isready",
                "-U",
                &config.user,
                "-d",
                &config.database,
            ])
            .output();

        if health.is_ok_and(|o| o.status.success()) {
            CliService::success("PostgreSQL is ready");
            return;
        }
    }

    CliService::warning("PostgreSQL started but health check timed out");
}
