//! docker-compose file generation for the local database.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;
use systemprompt_logging::CliService;
use systemprompt_models::defaults::DEFAULT_POSTGRES_PORT;

use super::postgres_access::PostgresConfig;

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

/// What one rendered `PostgreSQL` compose file says about its container.
///
/// `credentials` chooses between `${VAR}` interpolation (the local setup
/// passes them through `docker compose` env) and literal values (a tenant
/// container is generated with its own admin password); `container_name` and
/// `init_scripts` are present only for the named local database.
#[derive(Debug, Clone)]
pub struct PostgresCompose<'a> {
    pub host_port: u16,
    pub credentials: ComposeCredentials<'a>,
    pub container_name: Option<&'a str>,
    pub init_scripts: bool,
    pub restart: Option<&'a str>,
    pub header: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub enum ComposeCredentials<'a> {
    FromEnvironment,
    Literal {
        user: &'a str,
        password: &'a str,
        database: &'a str,
    },
}

#[must_use]
pub fn postgres_compose(opts: &PostgresCompose<'_>) -> String {
    let (user, password, database) = match opts.credentials {
        ComposeCredentials::FromEnvironment => (
            "${POSTGRES_USER:-systemprompt}".to_owned(),
            "${POSTGRES_PASSWORD}".to_owned(),
            "${POSTGRES_DB:-systemprompt}".to_owned(),
        ),
        ComposeCredentials::Literal {
            user,
            password,
            database,
        } => (user.to_owned(), password.to_owned(), database.to_owned()),
    };
    let data_volume = opts
        .container_name
        .map_or_else(|| "postgres_data".to_owned(), |name| format!("{name}_data"));
    let mut out = String::new();
    if let Some(header) = opts.header {
        out.push_str(header);
        out.push('\n');
    }
    out.push_str("services:\n  postgres:\n    image: postgres:18-alpine\n");
    if let Some(name) = opts.container_name {
        out.push_str(&format!("    container_name: {name}\n"));
    }
    if let Some(restart) = opts.restart {
        out.push_str(&format!("    restart: {restart}\n"));
    }
    out.push_str(&format!(
        "    environment:\n      POSTGRES_USER: {user}\n      POSTGRES_PASSWORD: {password}\n      \
         POSTGRES_DB: {database}\n    ports:\n      - \"{}:{DEFAULT_POSTGRES_PORT}\"\n    volumes:\n      - \
         {data_volume}:/var/lib/postgresql\n",
        opts.host_port
    ));
    if opts.init_scripts {
        out.push_str("      - ./init-scripts:/docker-entrypoint-initdb.d\n");
    }
    let ready = match opts.credentials {
        ComposeCredentials::FromEnvironment => format!("pg_isready -U {user}"),
        ComposeCredentials::Literal { .. } => format!("pg_isready -U {user} -d {database}"),
    };
    out.push_str(&format!(
        "    healthcheck:\n      test: [\"CMD-SHELL\", \"{ready}\"]\n      interval: 5s\n      timeout: \
         5s\n      retries: 5\n"
    ));
    if let Some(name) = opts.container_name {
        out.push_str(&format!(
            "    networks:\n      - {name}_network\n\nvolumes:\n  {data_volume}:\n\nnetworks:\n  \
             {name}_network:\n    driver: bridge\n"
        ));
    } else {
        out.push_str(&format!("\nvolumes:\n  {data_volume}: {{}}\n"));
    }
    out
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

    let compose_content = postgres_compose(&PostgresCompose {
        host_port: port,
        credentials: ComposeCredentials::FromEnvironment,
        container_name: Some(container_name),
        init_scripts: true,
        restart: None,
        header: None,
    });

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
