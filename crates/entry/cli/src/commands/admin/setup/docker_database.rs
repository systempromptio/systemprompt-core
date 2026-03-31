use anyhow::{Context, Result};
use std::process::Command;
use systemprompt_logging::CliService;

use super::postgres::PostgresConfig;

pub async fn create_database_in_docker(
    config: &PostgresConfig,
    container_name: &str,
) -> Result<()> {
    use sqlx::Row;
    use sqlx::postgres::PgPoolOptions;
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
