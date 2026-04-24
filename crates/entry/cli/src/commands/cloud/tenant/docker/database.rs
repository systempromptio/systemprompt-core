use anyhow::{Context, Result, bail};
use std::process::Command;
use systemprompt_logging::CliService;

use super::config::{SHARED_ADMIN_USER, SHARED_CONTAINER_NAME};

fn sanitize_database_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub fn create_database_for_tenant(admin_password: &str, port: u16, db_name: &str) -> Result<()> {
    let database_url = format!(
        "postgres://{}:{}@localhost:{}/postgres",
        SHARED_ADMIN_USER, admin_password, port
    );

    let safe_db_name = sanitize_database_name(db_name);

    let check_query = format!(
        "SELECT 1 FROM pg_database WHERE datname = '{}'",
        safe_db_name
    );
    let check_output = Command::new("docker")
        .args([
            "exec",
            SHARED_CONTAINER_NAME,
            "psql",
            &database_url,
            "-tAc",
            &check_query,
        ])
        .output()
        .with_context(|| {
            format!(
                "failed to run `docker exec {SHARED_CONTAINER_NAME} psql` checking for database \
                 {safe_db_name}"
            )
        })?;

    let exists = !String::from_utf8_lossy(&check_output.stdout)
        .trim()
        .is_empty();

    if exists {
        CliService::info(&format!("Database '{}' already exists", safe_db_name));
        return Ok(());
    }

    let create_query = format!("CREATE DATABASE \"{}\"", safe_db_name);
    let status = Command::new("docker")
        .args([
            "exec",
            SHARED_CONTAINER_NAME,
            "psql",
            &database_url,
            "-c",
            &create_query,
        ])
        .status()
        .with_context(|| {
            format!(
                "failed to run `docker exec {SHARED_CONTAINER_NAME} psql` creating database \
                 {safe_db_name}"
            )
        })?;

    if !status.success() {
        bail!("Failed to create database '{}'", safe_db_name);
    }

    Ok(())
}

pub fn drop_database_for_tenant(admin_password: &str, port: u16, db_name: &str) -> Result<()> {
    let database_url = format!(
        "postgres://{}:{}@localhost:{}/postgres",
        SHARED_ADMIN_USER, admin_password, port
    );

    let safe_db_name = sanitize_database_name(db_name);

    let terminate_query = format!(
        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{}' AND pid <> \
         pg_backend_pid()",
        safe_db_name
    );
    if let Err(e) = Command::new("docker")
        .args([
            "exec",
            SHARED_CONTAINER_NAME,
            "psql",
            &database_url,
            "-c",
            &terminate_query,
        ])
        .status()
    {
        tracing::debug!(
            error = %e,
            db = %safe_db_name,
            "failed to run `docker exec {SHARED_CONTAINER_NAME} psql` terminating existing connections",
        );
    }

    let drop_query = format!("DROP DATABASE IF EXISTS \"{}\"", safe_db_name);
    let status = Command::new("docker")
        .args([
            "exec",
            SHARED_CONTAINER_NAME,
            "psql",
            &database_url,
            "-c",
            &drop_query,
        ])
        .status()
        .with_context(|| {
            format!(
                "failed to run `docker exec {SHARED_CONTAINER_NAME} psql` dropping database \
                 {safe_db_name}"
            )
        })?;

    if !status.success() {
        bail!("Failed to drop database '{}'", safe_db_name);
    }

    Ok(())
}

pub fn ensure_admin_role(admin_password: &str) -> Result<()> {
    let role_check_query = format!(
        "SELECT 1 FROM pg_roles WHERE rolname = '{}'",
        SHARED_ADMIN_USER
    );
    let check_output = Command::new("docker")
        .args([
            "exec",
            SHARED_CONTAINER_NAME,
            "psql",
            "-U",
            SHARED_ADMIN_USER,
            "-d",
            "postgres",
            "-tAc",
            &role_check_query,
        ])
        .output()
        .with_context(|| {
            format!("failed to run `docker exec {SHARED_CONTAINER_NAME} psql` checking admin role")
        })?;

    let role_exists = !String::from_utf8_lossy(&check_output.stdout)
        .trim()
        .is_empty();

    if role_exists {
        let alter_password_sql = format!(
            "ALTER ROLE \"{}\" WITH PASSWORD '{}'",
            SHARED_ADMIN_USER,
            admin_password.replace('\'', "''")
        );
        let status = Command::new("docker")
            .args([
                "exec",
                SHARED_CONTAINER_NAME,
                "psql",
                "-U",
                SHARED_ADMIN_USER,
                "-d",
                "postgres",
                "-c",
                &alter_password_sql,
            ])
            .status()
            .with_context(|| {
                format!(
                    "failed to run `docker exec {SHARED_CONTAINER_NAME} psql` updating admin role \
                     password"
                )
            })?;

        if !status.success() {
            bail!("Failed to update password for role '{}'", SHARED_ADMIN_USER);
        }

        return Ok(());
    }

    let create_role_sql = format!(
        "CREATE ROLE \"{}\" WITH LOGIN CREATEDB SUPERUSER PASSWORD '{}'",
        SHARED_ADMIN_USER,
        admin_password.replace('\'', "''")
    );
    let status = Command::new("docker")
        .args([
            "exec",
            SHARED_CONTAINER_NAME,
            "psql",
            "-U",
            SHARED_ADMIN_USER,
            "-d",
            "postgres",
            "-c",
            &create_role_sql,
        ])
        .status()
        .with_context(|| {
            format!("failed to run `docker exec {SHARED_CONTAINER_NAME} psql` creating admin role")
        })?;

    if !status.success() {
        bail!("Failed to create role '{}'", SHARED_ADMIN_USER);
    }

    CliService::success(&format!("Created PostgreSQL role '{}'", SHARED_ADMIN_USER));
    Ok(())
}
