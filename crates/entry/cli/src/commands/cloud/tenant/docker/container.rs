use anyhow::{Context, Result, anyhow, bail};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use systemprompt_cloud::ProjectContext;
use systemprompt_logging::CliService;

use super::config::{SHARED_CONTAINER_NAME, SHARED_VOLUME_NAME, shared_config_path};

pub fn is_shared_container_running() -> bool {
    let output = Command::new("docker")
        .args(["ps", "-q", "-f", &format!("name={}", SHARED_CONTAINER_NAME)])
        .output();

    match output {
        Ok(out) => !String::from_utf8_lossy(&out.stdout).trim().is_empty(),
        Err(e) => {
            tracing::debug!(error = %e, "Failed to check shared container status");
            false
        },
    }
}

pub fn get_container_password() -> Option<String> {
    let output = Command::new("docker")
        .args([
            "inspect",
            SHARED_CONTAINER_NAME,
            "--format",
            "{{range .Config.Env}}{{println .}}{{end}}",
        ])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let env_vars = String::from_utf8_lossy(&out.stdout);
            for line in env_vars.lines() {
                if let Some(password) = line.strip_prefix("POSTGRES_PASSWORD=") {
                    return Some(password.to_string());
                }
            }
            None
        },
        Ok(_out) => {
            tracing::debug!("Docker inspect returned non-success exit code");
            None
        },
        Err(e) => {
            tracing::debug!(error = %e, "Failed to inspect container");
            None
        },
    }
}

pub fn check_volume_exists() -> bool {
    let output = Command::new("docker")
        .args([
            "volume",
            "ls",
            "-q",
            "-f",
            &format!("name={}", SHARED_VOLUME_NAME),
        ])
        .output();

    match output {
        Ok(out) => !String::from_utf8_lossy(&out.stdout).trim().is_empty(),
        Err(e) => {
            tracing::debug!(error = %e, "Failed to check volume existence");
            false
        },
    }
}

pub fn remove_shared_volume() -> Result<()> {
    let status = Command::new("docker")
        .args(["volume", "rm", SHARED_VOLUME_NAME])
        .status()
        .context("Failed to remove PostgreSQL volume")?;

    if !status.success() {
        bail!(
            "Failed to remove volume '{}'. Is a container still using it?",
            SHARED_VOLUME_NAME
        );
    }

    Ok(())
}

pub fn stop_shared_container() -> Result<()> {
    let ctx = ProjectContext::discover();
    let compose_path = ctx.docker_dir().join("shared.yaml");

    if compose_path.exists() {
        let compose_path_str = compose_path
            .to_str()
            .ok_or_else(|| anyhow!("Invalid compose path"))?;

        CliService::info("Stopping shared PostgreSQL container...");
        let status = Command::new("docker")
            .args(["compose", "-f", compose_path_str, "down", "-v"])
            .status()
            .context("Failed to stop shared container")?;

        if !status.success() {
            CliService::warning("Failed to stop container via compose, trying direct stop");
        }
    }

    let output = Command::new("docker")
        .args([
            "ps",
            "-aq",
            "-f",
            &format!("name={}", SHARED_CONTAINER_NAME),
        ])
        .output()?;

    let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !container_id.is_empty() {
        Command::new("docker")
            .args(["stop", &container_id])
            .status()?;
        Command::new("docker")
            .args(["rm", &container_id])
            .status()?;
    }

    let config_path = shared_config_path();
    if config_path.exists() {
        fs::remove_file(&config_path)?;
    }

    CliService::success("Shared PostgreSQL container removed");
    Ok(())
}

pub async fn wait_for_postgres_healthy(compose_path: &Path, timeout_secs: u64) -> Result<()> {
    let start = std::time::Instant::now();
    let compose_path_str = compose_path
        .to_str()
        .ok_or_else(|| anyhow!("Invalid compose path"))?;

    loop {
        let output = Command::new("docker")
            .args([
                "compose",
                "-f",
                compose_path_str,
                "ps",
                "--format",
                "{{.Health}}",
            ])
            .output()
            .context("Failed to check container health")?;

        let health = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if health.contains("healthy") {
            return Ok(());
        }

        if start.elapsed().as_secs() > timeout_secs {
            bail!(
                "Timeout waiting for PostgreSQL to become healthy.\nCheck logs with: docker \
                 compose -f {} logs",
                compose_path.display()
            );
        }

        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

pub fn generate_shared_postgres_compose(password: &str, port: u16) -> String {
    format!(
        r#"# systemprompt.io Shared PostgreSQL Container
# Generated by: systemprompt cloud tenant create
# Manage: docker compose -f .systemprompt/docker/shared.yaml up/down

services:
  postgres:
    image: postgres:18-alpine
    container_name: {container_name}
    restart: unless-stopped
    environment:
      POSTGRES_USER: {admin_user}
      POSTGRES_PASSWORD: {password}
      POSTGRES_DB: postgres
    ports:
      - "{port}:5432"
    volumes:
      - {volume_name}:/var/lib/postgresql
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U {admin_user}"]
      interval: 5s
      timeout: 5s
      retries: 5

volumes:
  {volume_name}:
    name: {volume_name}
"#,
        container_name = SHARED_CONTAINER_NAME,
        admin_user = super::config::SHARED_ADMIN_USER,
        password = password,
        port = port,
        volume_name = SHARED_VOLUME_NAME
    )
}

pub fn generate_admin_password() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |d| d.as_nanos());
    let random_part = format!("{:x}{:x}", timestamp, timestamp.wrapping_mul(31337));
    random_part.chars().take(32).collect()
}

pub fn nanoid() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |d| d.as_millis());
    format!("{:x}", timestamp)
}
