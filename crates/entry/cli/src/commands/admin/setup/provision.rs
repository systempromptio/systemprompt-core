//! Role and database provisioning shared by every setup path.
//!
//! One superuser connection creates the role (or sets an existing role's
//! password to the one the profile will carry), creates the database and
//! grants the role on it, so the role and the written profile never disagree.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use sqlx::PgPool;
use systemprompt_logging::CliService;

use super::ddl;
use super::postgres_access::PostgresConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleOutcome {
    Created,
    PasswordSet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProvisionOutcome {
    pub role: RoleOutcome,
    pub database_created: bool,
}

pub async fn provision_role_and_db(
    pool: &PgPool,
    config: &PostgresConfig,
) -> Result<ProvisionOutcome> {
    let role_exists = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM pg_roles WHERE rolname = $1)",
        &config.user
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(false);

    let (role_sql, role) = if role_exists {
        (
            ddl::build_alter_user_password_sql(&config.user, &config.password),
            RoleOutcome::PasswordSet,
        )
    } else {
        (
            ddl::build_create_user_sql(&config.user, &config.password),
            RoleOutcome::Created,
        )
    };
    sqlx::query(sqlx::AssertSqlSafe(role_sql))
        .execute(pool)
        .await
        .with_context(|| format!("Failed to provision role '{}'", config.user))?;

    let db_exists = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)",
        &config.database
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(false);

    if !db_exists {
        sqlx::query(sqlx::AssertSqlSafe(ddl::build_create_db_sql(
            &config.database,
            &config.user,
        )))
        .execute(pool)
        .await
        .with_context(|| format!("Failed to create database '{}'", config.database))?;
    }

    sqlx::query(sqlx::AssertSqlSafe(ddl::build_grant_sql(
        &config.database,
        &config.user,
    )))
    .execute(pool)
    .await
    .with_context(|| format!("Failed to grant '{}' on '{}'", config.user, config.database))?;

    Ok(ProvisionOutcome {
        role,
        database_created: !db_exists,
    })
}

pub fn report(config: &PostgresConfig, outcome: ProvisionOutcome) {
    match outcome.role {
        RoleOutcome::Created => CliService::success(&format!("Created user '{}'", config.user)),
        RoleOutcome::PasswordSet => CliService::success(&format!(
            "User '{}' already existed; its password now matches the profile",
            config.user
        )),
    }
    if outcome.database_created {
        CliService::success(&format!("Created database '{}'", config.database));
    }
}
