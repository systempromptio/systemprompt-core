//! `PostgreSQL` provisioning steps for the setup wizard.
//!
//! Resolves connection parameters from [`SetupArgs`], reaches an existing
//! server or starts one via Docker, and—when given superuser
//! credentials—creates the role, database, and grants. The bootstrap `CREATE
//! USER`/`CREATE DATABASE`/ `GRANT` statements use dynamic SQL because they run
//! before the target database exists and cannot bind parameters.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow};
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use systemprompt_logging::CliService;

use super::SetupArgs;
use super::common::{
    PostgresConfig, detect_postgresql, enable_extensions, generate_password, test_connection,
};
use crate::CliConfig;
use crate::interactive::Prompter;

pub async fn setup_non_interactive(
    args: &SetupArgs,
    env_name: &str,
    cli_config: &CliConfig,
) -> Result<PostgresConfig> {
    if !cli_config.is_json_output() {
        CliService::section(&format!("PostgreSQL Setup ({})", env_name));
    }

    let password = args.db_password.clone().unwrap_or_else(generate_password);
    let config = PostgresConfig {
        host: args.db_host.clone(),
        port: args.db_port,
        user: args.effective_db_user(env_name),
        password,
        database: args.effective_db_name(env_name),
    };

    if !cli_config.is_json_output() {
        CliService::key_value("Host", &config.host);
        CliService::key_value("Port", &config.port.to_string());
        CliService::key_value("User", &config.user);
        CliService::key_value("Database", &config.database);
    }

    if args.docker {
        if !cli_config.is_json_output() {
            CliService::info("Setting up PostgreSQL with Docker...");
        }
        return super::docker::setup_docker_postgres_non_interactive(&config, env_name).await;
    }

    if detect_postgresql(&config.host, config.port) {
        if !cli_config.is_json_output() {
            CliService::success(&format!(
                "PostgreSQL reachable at {}:{}",
                config.host, config.port
            ));
        }
    } else if !cli_config.is_json_output() {
        CliService::warning(&format!(
            "PostgreSQL not reachable at {}:{}",
            config.host, config.port
        ));
        CliService::info("Continuing with provided configuration...");
    }

    if test_connection(&config).await {
        if !cli_config.is_json_output() {
            CliService::success("Database connection successful");
        }
        enable_extensions(&config).await?;
    } else if !cli_config.is_json_output() {
        CliService::warning("Cannot connect to database - it may need to be created manually");
    }

    Ok(config)
}

pub async fn setup_interactive(
    args: &SetupArgs,
    prompter: &dyn Prompter,
    env_name: &str,
    _cli_config: &CliConfig,
) -> Result<PostgresConfig> {
    CliService::section(&format!("PostgreSQL Setup ({})", env_name));
    CliService::info("Configure PostgreSQL database for your local environment.");

    let options = vec![
        "Use existing PostgreSQL installation".to_owned(),
        "Start PostgreSQL with Docker".to_owned(),
    ];

    let selection = prompter.select("How would you like to set up PostgreSQL?", &options)?;

    match selection {
        0 => setup_existing_postgres(args, prompter, env_name).await,
        1 => super::docker::setup_docker_postgres_interactive(args, prompter, env_name).await,
        _ => Err(anyhow!("Invalid PostgreSQL setup option selected")),
    }
}

async fn setup_existing_postgres(
    args: &SetupArgs,
    prompter: &dyn Prompter,
    env_name: &str,
) -> Result<PostgresConfig> {
    CliService::info("Configuring existing PostgreSQL connection...");

    let (host, port) = prompt_host_port(args, prompter)?;

    let user = prompter.input_with_default("Database user", &args.effective_db_user(env_name))?;

    let password = prompt_password(args, prompter)?;

    let database =
        prompter.input_with_default("Database name", &args.effective_db_name(env_name))?;

    let config = PostgresConfig {
        host,
        port,
        user,
        password,
        database,
    };

    verify_or_create_database(&config, prompter).await?;

    Ok(config)
}

fn prompt_host_port(args: &SetupArgs, prompter: &dyn Prompter) -> Result<(String, u16)> {
    let host = prompter.input_with_default("PostgreSQL host", &args.db_host)?;

    let port_input = prompter.input_with_default("PostgreSQL port", &args.db_port.to_string())?;
    let port: u16 = port_input
        .trim()
        .parse()
        .with_context(|| format!("Invalid PostgreSQL port: {}", port_input))?;

    if detect_postgresql(&host, port) {
        CliService::success(&format!("PostgreSQL reachable at {}:{}", host, port));
    } else {
        CliService::warning(&format!("Cannot reach PostgreSQL at {}:{}", host, port));
        let continue_anyway = prompter.confirm("Continue anyway?", false)?;

        if !continue_anyway {
            anyhow::bail!("PostgreSQL not reachable. Please start PostgreSQL and try again.");
        }
    }

    Ok((host, port))
}

fn prompt_password(args: &SetupArgs, prompter: &dyn Prompter) -> Result<String> {
    let password = if let Some(ref pw) = args.db_password {
        pw.clone()
    } else {
        let use_generated = prompter.confirm("Generate a secure password automatically?", true)?;

        if use_generated {
            let generated = generate_password();
            CliService::success(&format!("Generated password: {}", generated));
            generated
        } else {
            prompter.password("Database password")?
        }
    };

    if password.is_empty() {
        anyhow::bail!("Password is required");
    }

    Ok(password)
}

async fn verify_or_create_database(config: &PostgresConfig, prompter: &dyn Prompter) -> Result<()> {
    if test_connection(config).await {
        CliService::success("Successfully connected to database!");
        enable_extensions(config).await?;
        return Ok(());
    }

    CliService::warning("Cannot connect with provided credentials.");
    CliService::info("The database or user may not exist yet.");

    let create_db = prompter.confirm("Create database and user now? (requires superuser)", true)?;

    if create_db {
        create_database_interactive(config, prompter).await?;
        enable_extensions(config).await?;
    } else {
        CliService::warning("Skipping database creation. You may need to create it manually.");
    }

    Ok(())
}

async fn create_database_interactive(
    config: &PostgresConfig,
    prompter: &dyn Prompter,
) -> Result<()> {
    CliService::info("Enter PostgreSQL superuser credentials (typically 'postgres'):");

    let superuser = prompter.input_with_default("Superuser name", "postgres")?;

    let superpass = prompter.password("Superuser password")?;

    if superpass.is_empty() {
        anyhow::bail!("Superuser password is required");
    }

    let super_url = format!(
        "postgres://{}:{}@{}:{}/postgres",
        superuser, superpass, config.host, config.port
    );

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&super_url)
        .await
        .context("Failed to connect with superuser credentials")?;

    let user_exists: bool = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM pg_roles WHERE rolname = $1)",
        &config.user
    )
    .fetch_one(&pool)
    .await?
    .unwrap_or(false);

    if !user_exists {
        CliService::info(&format!("Creating user '{}'...", config.user));
        let create_user_sql = super::ddl::build_create_user_sql(&config.user, &config.password);
        sqlx::query(sqlx::AssertSqlSafe(create_user_sql))
            .execute(&pool)
            .await?;
        CliService::success(&format!("Created user '{}'", config.user));
    }

    let db_exists: bool = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)",
        &config.database
    )
    .fetch_one(&pool)
    .await?
    .unwrap_or(false);

    if !db_exists {
        CliService::info(&format!("Creating database '{}'...", config.database));
        let create_db_sql = super::ddl::build_create_db_sql(&config.database, &config.user);
        sqlx::query(sqlx::AssertSqlSafe(create_db_sql))
            .execute(&pool)
            .await?;
        CliService::success(&format!("Created database '{}'", config.database));
    }

    let grant_sql = super::ddl::build_grant_sql(&config.database, &config.user);
    sqlx::query(sqlx::AssertSqlSafe(grant_sql))
        .execute(&pool)
        .await?;

    pool.close().await;

    CliService::success("Database and user setup complete");
    Ok(())
}
