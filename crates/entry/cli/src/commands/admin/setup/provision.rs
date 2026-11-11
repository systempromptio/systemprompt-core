//! Role and database provisioning shared by every setup path.
//!
//! One superuser connection creates the role, creates the database and grants
//! the role on it. An existing role is never changed unless the operator asked
//! for `--reset-role-password`: its password must log in, or setup stops, so
//! the role and the written profile never disagree. A superuser role is
//! refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, bail};
use sqlx::PgPool;
use systemprompt_logging::CliService;

use super::ddl;
use super::postgres_access::{PostgresConfig, connect};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleOutcome {
    Created,
    Verified,
    PasswordReset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProvisionOutcome {
    pub role: RoleOutcome,
    pub database_created: bool,
}

pub async fn provision_role_and_db(
    pool: &PgPool,
    config: &PostgresConfig,
    reset_role_password: bool,
) -> Result<ProvisionOutcome> {
    let existing = sqlx::query_scalar!(
        r#"SELECT rolsuper AS "rolsuper!" FROM pg_roles WHERE rolname = $1"#,
        &config.user
    )
    .fetch_optional(pool)
    .await?;

    let role = match existing {
        None => {
            run_ddl(
                pool,
                ddl::build_create_user_sql(&config.user, &config.password),
                config,
            )
            .await?;
            RoleOutcome::Created
        },
        Some(true) => bail!(
            "Role '{}' is a superuser; setup neither verifies nor changes a superuser role. \
             Choose a dedicated role.",
            config.user
        ),
        Some(false) if reset_role_password => {
            run_ddl(
                pool,
                ddl::build_alter_user_password_sql(&config.user, &config.password),
                config,
            )
            .await?;
            RoleOutcome::PasswordReset
        },
        Some(false) => {
            if !role_login_succeeds(config).await {
                bail!(
                    "Role '{}' already exists and the supplied password does not log in as it. \
                     Supply that role's password, or pass --reset-role-password to change it \
                     (anything else using the role loses access). Nothing was written.",
                    config.user
                );
            }
            RoleOutcome::Verified
        },
    };

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
        RoleOutcome::Verified => CliService::success(&format!(
            "User '{}' already existed; the supplied password logs in as it",
            config.user
        )),
        RoleOutcome::PasswordReset => CliService::warning(&format!(
            "User '{}' already existed; its password was reset (--reset-role-password)",
            config.user
        )),
    }
    if outcome.database_created {
        CliService::success(&format!("Created database '{}'", config.database));
    }
}

async fn run_ddl(pool: &PgPool, sql: String, config: &PostgresConfig) -> Result<()> {
    sqlx::query(sqlx::AssertSqlSafe(sql))
        .execute(pool)
        .await
        .with_context(|| format!("Failed to provision role '{}'", config.user))?;
    Ok(())
}

async fn role_login_succeeds(config: &PostgresConfig) -> bool {
    let options = config.connect_options().database("postgres");
    match connect(options).await {
        Ok(pool) => {
            pool.close().await;
            true
        },
        Err(_) => false,
    }
}
