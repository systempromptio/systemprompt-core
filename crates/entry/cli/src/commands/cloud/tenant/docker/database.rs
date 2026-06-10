//! Per-tenant database operations inside the shared `PostgreSQL` container.
//!
//! Creates, drops, and authorises tenant databases by running `psql` via
//! `docker exec`, sanitising identifiers before they reach the SQL text.

use anyhow::{Context, Result, bail};
use systemprompt_cloud::DockerCli;
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

pub(in crate::commands::cloud::tenant) fn create_database_for_tenant(
    admin_password: &str,
    port: u16,
    db_name: &str,
) -> Result<()> {
    let database_url = format!(
        "postgres://{}:{}@localhost:{}/postgres",
        SHARED_ADMIN_USER, admin_password, port
    );

    let safe_db_name = sanitize_database_name(db_name);

    let check_query = format!(
        "SELECT 1 FROM pg_database WHERE datname = '{}'",
        safe_db_name
    );
    let check_output = DockerCli::new()
        .output(&[
            "exec",
            SHARED_CONTAINER_NAME,
            "psql",
            &database_url,
            "-tAc",
            &check_query,
        ])
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
    let status = DockerCli::new()
        .status(&[
            "exec",
            SHARED_CONTAINER_NAME,
            "psql",
            &database_url,
            "-c",
            &create_query,
        ])
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

pub(in crate::commands::cloud::tenant) fn drop_database_for_tenant(
    admin_password: &str,
    port: u16,
    db_name: &str,
) -> Result<()> {
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
    if let Err(e) = DockerCli::new().status(&[
        "exec",
        SHARED_CONTAINER_NAME,
        "psql",
        &database_url,
        "-c",
        &terminate_query,
    ]) {
        tracing::debug!(
            error = %e,
            db = %safe_db_name,
            "failed to run `docker exec {SHARED_CONTAINER_NAME} psql` terminating existing connections",
        );
    }

    let drop_query = format!("DROP DATABASE IF EXISTS \"{}\"", safe_db_name);
    let status = DockerCli::new()
        .status(&[
            "exec",
            SHARED_CONTAINER_NAME,
            "psql",
            &database_url,
            "-c",
            &drop_query,
        ])
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

pub(in crate::commands::cloud::tenant) fn ensure_admin_role(admin_password: &str) -> Result<()> {
    let role_check_query = format!(
        "SELECT 1 FROM pg_roles WHERE rolname = '{}'",
        SHARED_ADMIN_USER
    );
    let role_exists = !admin_psql_capture(&role_check_query, "checking admin role")?.is_empty();

    if role_exists {
        let alter_password_sql = format!(
            "ALTER ROLE \"{}\" WITH PASSWORD '{}'",
            SHARED_ADMIN_USER,
            admin_password.replace('\'', "''")
        );
        if !admin_psql_execute(&alter_password_sql, "updating admin role password")? {
            bail!("Failed to update password for role '{}'", SHARED_ADMIN_USER);
        }

        return Ok(());
    }

    let create_role_sql = format!(
        "CREATE ROLE \"{}\" WITH LOGIN CREATEDB SUPERUSER PASSWORD '{}'",
        SHARED_ADMIN_USER,
        admin_password.replace('\'', "''")
    );
    if !admin_psql_execute(&create_role_sql, "creating admin role")? {
        bail!("Failed to create role '{}'", SHARED_ADMIN_USER);
    }

    CliService::success(&format!("Created PostgreSQL role '{}'", SHARED_ADMIN_USER));
    Ok(())
}

fn admin_psql_capture(sql: &str, action: &str) -> Result<String> {
    let output = DockerCli::new()
        .output(&[
            "exec",
            SHARED_CONTAINER_NAME,
            "psql",
            "-U",
            SHARED_ADMIN_USER,
            "-d",
            "postgres",
            "-tAc",
            sql,
        ])
        .with_context(|| {
            format!("failed to run `docker exec {SHARED_CONTAINER_NAME} psql` {action}")
        })?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn admin_psql_execute(sql: &str, action: &str) -> Result<bool> {
    let status = DockerCli::new()
        .status(&[
            "exec",
            SHARED_CONTAINER_NAME,
            "psql",
            "-U",
            SHARED_ADMIN_USER,
            "-d",
            "postgres",
            "-c",
            sql,
        ])
        .with_context(|| {
            format!("failed to run `docker exec {SHARED_CONTAINER_NAME} psql` {action}")
        })?;

    Ok(status.success())
}
