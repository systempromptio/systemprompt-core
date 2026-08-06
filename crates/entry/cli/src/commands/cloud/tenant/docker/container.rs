//! Lifecycle of a tenant's own Docker `PostgreSQL` container.
//!
//! Each local tenant owns a compose project under `.systemprompt/docker/`, so
//! two installations on one host never share a container, a volume, or a role.
//! Wraps `docker compose` to bring a project up, health-check it, and tear it
//! down with its volume.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow, bail};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use systemprompt_cloud::{DockerCli, ProjectContext};

const LOCAL_DB_USER: &str = "systemprompt";
const LOCAL_DB_NAME: &str = "systemprompt";

#[derive(Debug, Clone)]
pub struct TenantContainer {
    pub project: String,
    pub password: String,
    pub port: u16,
}

impl TenantContainer {
    #[must_use]
    pub const fn new(project: String, password: String, port: u16) -> Self {
        Self {
            project,
            password,
            port,
        }
    }

    #[must_use]
    pub fn compose_path(&self) -> PathBuf {
        ProjectContext::discover()
            .docker_dir()
            .join(format!("{}.yaml", self.project))
    }

    #[must_use]
    pub fn database_url(&self) -> String {
        format!(
            "postgres://{}:{}@localhost:{}/{}",
            LOCAL_DB_USER, self.password, self.port, LOCAL_DB_NAME
        )
    }
}

pub fn compose_path_for_project(project: &str) -> PathBuf {
    ProjectContext::discover()
        .docker_dir()
        .join(format!("{project}.yaml"))
}

pub fn is_project_running(docker: &DockerCli, project: &str) -> bool {
    let filter = format!("label=com.docker.compose.project={project}");
    match docker.output(&["ps", "-q", "-f", &filter]) {
        Ok(out) => !String::from_utf8_lossy(&out.stdout).trim().is_empty(),
        Err(e) => {
            tracing::debug!(error = %e, project = %project, "Failed to check container status");
            false
        },
    }
}

pub async fn start_project(docker: &DockerCli, container: &TenantContainer) -> Result<()> {
    let compose_path = container.compose_path();
    if let Some(parent) = compose_path.parent() {
        fs::create_dir_all(parent).context("Failed to create docker directory")?;
    }

    fs::write(
        &compose_path,
        generate_postgres_compose(&container.password, container.port),
    )
    .with_context(|| format!("Failed to write {}", compose_path.display()))?;

    let compose_path_str = compose_path
        .to_str()
        .ok_or_else(|| anyhow!("Invalid compose path"))?;

    let status = docker
        .status(&[
            "compose",
            "-p",
            &container.project,
            "-f",
            compose_path_str,
            "up",
            "-d",
        ])
        .context("Failed to execute docker compose. Is Docker running?")?;

    if !status.success() {
        bail!("Failed to start PostgreSQL container. Is Docker running?");
    }

    wait_for_postgres_healthy(docker, &container.project, &compose_path, 60).await
}

pub fn remove_project(docker: &DockerCli, project: &str) -> Result<()> {
    let compose_path = compose_path_for_project(project);

    if compose_path.exists() {
        let compose_path_str = compose_path
            .to_str()
            .ok_or_else(|| anyhow!("Invalid compose path"))?;

        let status = docker
            .status(&[
                "compose",
                "-p",
                project,
                "-f",
                compose_path_str,
                "down",
                "-v",
            ])
            .context("Failed to stop tenant container")?;

        if !status.success() {
            bail!("Failed to remove container for project '{project}'");
        }

        fs::remove_file(&compose_path)
            .with_context(|| format!("Failed to remove {}", compose_path.display()))?;
    }

    Ok(())
}

pub async fn wait_for_postgres_healthy(
    docker: &DockerCli,
    project: &str,
    compose_path: &Path,
    timeout_secs: u64,
) -> Result<()> {
    let start = std::time::Instant::now();
    let compose_path_str = compose_path
        .to_str()
        .ok_or_else(|| anyhow!("Invalid compose path"))?;

    loop {
        let output = docker
            .output(&[
                "compose",
                "-p",
                project,
                "-f",
                compose_path_str,
                "ps",
                "--format",
                "{{.Health}}",
            ])
            .context("Failed to check container health")?;

        let health = String::from_utf8_lossy(&output.stdout).trim().to_owned();

        if health.contains("healthy") {
            return Ok(());
        }

        if start.elapsed().as_secs() > timeout_secs {
            bail!(
                "Timeout waiting for PostgreSQL to become healthy.\nCheck logs with: docker \
                 compose -p {} -f {} logs",
                project,
                compose_path.display()
            );
        }

        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

fn generate_postgres_compose(password: &str, port: u16) -> String {
    format!(
        r#"# systemprompt.io tenant PostgreSQL container
# Generated by: systemprompt cloud tenant create
# Manage with the project name this file was created under.

services:
  postgres:
    image: postgres:18-alpine
    restart: unless-stopped
    environment:
      POSTGRES_USER: {LOCAL_DB_USER}
      POSTGRES_PASSWORD: {password}
      POSTGRES_DB: {LOCAL_DB_NAME}
    ports:
      - "{port}:5432"
    volumes:
      - postgres_data:/var/lib/postgresql
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U {LOCAL_DB_USER} -d {LOCAL_DB_NAME}"]
      interval: 5s
      timeout: 5s
      retries: 5

volumes:
  postgres_data: {{}}
"#
    )
}

pub(in crate::commands::cloud) fn generate_admin_password() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |d| d.as_nanos());
    let random_part = format!("{:x}{:x}", timestamp, timestamp.wrapping_mul(31337));
    random_part.chars().take(32).collect()
}

pub(in crate::commands::cloud) fn nanoid() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |d| d.as_millis());
    format!("{timestamp:x}")
}

#[must_use]
pub(in crate::commands::cloud) fn new_local_tenant_id() -> systemprompt_identifiers::TenantId {
    systemprompt_identifiers::TenantId::new(format!("local_{}", uuid::Uuid::new_v4()))
}
