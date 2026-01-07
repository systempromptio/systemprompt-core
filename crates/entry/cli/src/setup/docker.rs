use anyhow::{Context, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input};
use std::process::Command;
use systemprompt_cloud::constants::docker::{container_name, COMPOSE_PATH};
use systemprompt_core_logging::CliService;

use super::postgres::{generate_password, PostgresConfig};

pub async fn setup_docker_postgres(env_name: &str) -> Result<PostgresConfig> {
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

    let default_user = format!("systemprompt_{}", env_name);
    let user: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Database user")
        .default(default_user)
        .interact_text()?;

    let password = generate_password();
    CliService::success(&format!("Generated password: {}", password));

    let default_db = format!("systemprompt_{}", env_name);
    let database: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Database name")
        .default(default_db)
        .interact_text()?;

    let port: u16 = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("PostgreSQL port")
        .default(5432u16)
        .interact_text()?;

    let config = PostgresConfig {
        host: "localhost".to_string(),
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

        let reuse = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Use existing container?")
            .default(true)
            .interact()?;

        if reuse {
            if !super::postgres::test_connection(&config).await {
                CliService::info("Creating database and user in existing container...");
                create_database_in_docker(&config, &container).await?;
            }
            super::postgres::enable_extensions(&config).await?;
            return Ok(config);
        }

        CliService::info("Stopping existing container...");
        let _ = Command::new("docker").args(["stop", &container]).output();
        let _ = Command::new("docker").args(["rm", &container]).output();
    }

    start_compose(&config, &compose_dir, &container)?;

    wait_for_postgres_ready(&config, &container)?;

    super::postgres::enable_extensions(&config).await?;

    Ok(config)
}

pub fn is_docker_available() -> bool {
    Command::new("docker").arg("--version").output().is_ok()
}

pub fn is_compose_available() -> bool {
    Command::new("docker")
        .args(["compose", "version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
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
        .map(|o| !String::from_utf8_lossy(&o.stdout).trim().is_empty())
        .unwrap_or(false)
}

fn create_compose_files_if_missing(
    compose_dir: &std::path::Path,
    container_name: &str,
    port: u16,
) -> Result<()> {
    let compose_file = compose_dir.join("docker-compose.yaml");

    std::fs::create_dir_all(compose_dir).context("Failed to create infrastructure/docker")?;

    let init_scripts_dir = compose_dir.join("init-scripts");
    std::fs::create_dir_all(&init_scripts_dir).context("Failed to create init-scripts")?;

    let compose_content = format!(
        r#"services:
  postgres:
    image: postgres:16-alpine
    container_name: {container_name}
    environment:
      POSTGRES_USER: ${{POSTGRES_USER:-systemprompt}}
      POSTGRES_PASSWORD: ${{POSTGRES_PASSWORD}}
      POSTGRES_DB: ${{POSTGRES_DB:-systemprompt}}
    ports:
      - "{port}:5432"
    volumes:
      - {container_name}_data:/var/lib/postgresql/data
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

    const INIT_SCRIPT: &str = r#"CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";
"#;

    std::fs::write(&compose_file, compose_content)
        .context("Failed to write docker-compose.yaml")?;

    std::fs::write(init_scripts_dir.join("01-extensions.sql"), INIT_SCRIPT)
        .context("Failed to write init script")?;

    CliService::success(&format!("Created {}", compose_file.display()));

    Ok(())
}

fn start_compose(
    config: &PostgresConfig,
    compose_dir: &std::path::Path,
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

fn wait_for_postgres_ready(config: &PostgresConfig, container_name: &str) -> Result<()> {
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

        if health.map(|o| o.status.success()).unwrap_or(false) {
            CliService::success("PostgreSQL is ready");
            return Ok(());
        }
    }

    CliService::warning("PostgreSQL started but health check timed out");
    Ok(())
}

pub async fn create_database_in_docker(
    config: &PostgresConfig,
    container_name: &str,
) -> Result<()> {
    use sqlx::postgres::PgPoolOptions;
    use sqlx::Row;
    use std::time::Duration;

    CliService::info("Creating database and user in Docker container...");

    let output = Command::new("docker")
        .args(["exec", container_name, "printenv", "POSTGRES_PASSWORD"])
        .output()
        .context("Failed to get container password")?;

    let container_password = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if container_password.is_empty() {
        anyhow::bail!(
            "Could not get container password. The container may not have POSTGRES_PASSWORD set."
        );
    }

    let super_url = format!(
        "postgres://postgres:{}@{}:{}/postgres",
        container_password, config.host, config.port
    );

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&super_url)
        .await
        .context("Failed to connect to Docker PostgreSQL")?;

    let user_exists: bool = sqlx::query("SELECT EXISTS(SELECT 1 FROM pg_roles WHERE rolname = $1)")
        .bind(&config.user)
        .fetch_one(&pool)
        .await?
        .get(0);

    if !user_exists {
        let create_user_sql = format!(
            "CREATE USER \"{}\" WITH PASSWORD '{}'",
            config.user.replace('"', "\"\""),
            config.password.replace('\'', "''")
        );
        sqlx::query(&create_user_sql).execute(&pool).await?;
        CliService::success(&format!("Created user '{}'", config.user));
    }

    let db_exists: bool =
        sqlx::query("SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)")
            .bind(&config.database)
            .fetch_one(&pool)
            .await?
            .get(0);

    if !db_exists {
        let create_db_sql = format!(
            "CREATE DATABASE \"{}\" OWNER \"{}\"",
            config.database.replace('"', "\"\""),
            config.user.replace('"', "\"\"")
        );
        sqlx::query(&create_db_sql).execute(&pool).await?;
        CliService::success(&format!("Created database '{}'", config.database));
    }

    pool.close().await;
    Ok(())
}
