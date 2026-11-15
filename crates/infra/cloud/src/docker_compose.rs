//! docker-compose file generation for the local database.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};
use std::process::Command;
use systemprompt_extension::cli::ChildSupervisor;
use systemprompt_logging::CliService;
use systemprompt_models::defaults::DEFAULT_POSTGRES_PORT;

/// The database credentials a compose file or container command carries.
#[derive(Debug, Clone, Copy)]
pub struct PostgresCredentials<'a> {
    pub user: &'a str,
    pub password: &'a str,
    pub database: &'a str,
}

#[derive(Debug, thiserror::Error)]
pub enum DockerComposeError {
    #[error("Failed to create {}", path.display())]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to write {}", path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to run docker compose")]
    Run(#[source] std::io::Error),
    #[error("Failed to start PostgreSQL: {stderr}")]
    Start { stderr: String },
}

pub fn is_docker_available(children: &dyn ChildSupervisor) -> bool {
    let mut cmd = Command::new("docker");
    cmd.arg("--version");
    children.output(cmd).is_ok()
}

pub fn is_compose_available(children: &dyn ChildSupervisor) -> bool {
    let mut cmd = Command::new("docker");
    cmd.args(["compose", "version"]);
    children.output(cmd).is_ok_and(|o| o.status.success())
}

pub fn is_container_running(children: &dyn ChildSupervisor, container_name: &str) -> bool {
    let mut cmd = Command::new("docker");
    cmd.args([
        "ps",
        "--filter",
        &format!("name=^{}$", container_name),
        "--format",
        "{{.Names}}",
    ]);
    children
        .output(cmd)
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
    Literal(PostgresCredentials<'a>),
}

#[must_use]
pub fn postgres_compose(opts: &PostgresCompose<'_>) -> String {
    let (user, password, database) = match opts.credentials {
        ComposeCredentials::FromEnvironment => (
            "${POSTGRES_USER:-systemprompt}".to_owned(),
            "${POSTGRES_PASSWORD}".to_owned(),
            "${POSTGRES_DB:-systemprompt}".to_owned(),
        ),
        ComposeCredentials::Literal(credentials) => (
            credentials.user.to_owned(),
            credentials.password.to_owned(),
            credentials.database.to_owned(),
        ),
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
        ComposeCredentials::Literal(_) => format!("pg_isready -U {user} -d {database}"),
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
) -> Result<(), DockerComposeError> {
    const INIT_SCRIPT: &str = r#"CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";
"#;

    let compose_file = compose_dir.join("docker-compose.yaml");

    std::fs::create_dir_all(compose_dir).map_err(|source| DockerComposeError::CreateDir {
        path: compose_dir.to_path_buf(),
        source,
    })?;

    let init_scripts_dir = compose_dir.join("init-scripts");
    std::fs::create_dir_all(&init_scripts_dir).map_err(|source| {
        DockerComposeError::CreateDir {
            path: init_scripts_dir.clone(),
            source,
        }
    })?;

    let compose_content = postgres_compose(&PostgresCompose {
        host_port: port,
        credentials: ComposeCredentials::FromEnvironment,
        container_name: Some(container_name),
        init_scripts: true,
        restart: None,
        header: None,
    });

    std::fs::write(&compose_file, compose_content).map_err(|source| DockerComposeError::Write {
        path: compose_file.clone(),
        source,
    })?;

    let init_script = init_scripts_dir.join("01-extensions.sql");
    std::fs::write(&init_script, INIT_SCRIPT).map_err(|source| DockerComposeError::Write {
        path: init_script,
        source,
    })?;

    CliService::success(&format!("Created {}", compose_file.display()));

    Ok(())
}

pub fn start_compose(
    children: &dyn ChildSupervisor,
    credentials: PostgresCredentials<'_>,
    compose_dir: &Path,
    container_name: &str,
) -> Result<(), DockerComposeError> {
    CliService::info("Starting PostgreSQL...");

    let mut cmd = Command::new("docker");
    cmd.args(["compose", "up", "-d"])
        .current_dir(compose_dir)
        .env("POSTGRES_USER", credentials.user)
        .env("POSTGRES_PASSWORD", credentials.password)
        .env("POSTGRES_DB", credentials.database)
        .env("CONTAINER_NAME", container_name);
    let result = children.output(cmd).map_err(DockerComposeError::Run)?;

    if !result.status.success() {
        return Err(DockerComposeError::Start {
            stderr: String::from_utf8_lossy(&result.stderr).into_owned(),
        });
    }

    CliService::success("PostgreSQL container started");
    Ok(())
}

pub fn wait_for_postgres_ready(
    children: &dyn ChildSupervisor,
    credentials: PostgresCredentials<'_>,
    container_name: &str,
) {
    CliService::info("Waiting for PostgreSQL to be ready...");

    for _ in 0..30 {
        std::thread::sleep(std::time::Duration::from_secs(1));

        let mut cmd = Command::new("docker");
        cmd.args([
            "exec",
            container_name,
            "pg_isready",
            "-U",
            credentials.user,
            "-d",
            credentials.database,
        ]);
        let health = children.output(cmd);

        if health.is_ok_and(|o| o.status.success()) {
            CliService::success("PostgreSQL is ready");
            return;
        }
    }

    CliService::warning("PostgreSQL started but health check timed out");
}
