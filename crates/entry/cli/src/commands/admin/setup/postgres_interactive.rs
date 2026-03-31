use anyhow::{Context, Result, anyhow};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input, Password, Select};
use sqlx::Row;
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use systemprompt_logging::CliService;

use super::SetupArgs;
use super::postgres::{PostgresConfig, detect_postgresql, enable_extensions, generate_password, test_connection};
use crate::CliConfig;

pub async fn setup_interactive(
    args: &SetupArgs,
    env_name: &str,
    _cli_config: &CliConfig,
) -> Result<PostgresConfig> {
    CliService::section(&format!("PostgreSQL Setup ({})", env_name));
    CliService::info("Configure PostgreSQL database for your local environment.");

    let options = vec![
        "Use existing PostgreSQL installation",
        "Start PostgreSQL with Docker",
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("How would you like to set up PostgreSQL?")
        .items(&options)
        .default(usize::from(args.docker))
        .interact()?;

    match selection {
        0 => setup_existing_postgres(args, env_name).await,
        1 => super::docker::setup_docker_postgres_interactive(args, env_name).await,
        _ => Err(anyhow!("Invalid PostgreSQL setup option selected")),
    }
}

async fn setup_existing_postgres(args: &SetupArgs, env_name: &str) -> Result<PostgresConfig> {
    CliService::info("Configuring existing PostgreSQL connection...");

    let host: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("PostgreSQL host")
        .default(args.db_host.clone())
        .interact_text()?;

    let port: u16 = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("PostgreSQL port")
        .default(args.db_port)
        .interact_text()?;

    if detect_postgresql(&host, port) {
        CliService::success(&format!("PostgreSQL reachable at {}:{}", host, port));
    } else {
        CliService::warning(&format!("Cannot reach PostgreSQL at {}:{}", host, port));
        let continue_anyway = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Continue anyway?")
            .default(false)
            .interact()?;

        if !continue_anyway {
            anyhow::bail!("PostgreSQL not reachable. Please start PostgreSQL and try again.");
        }
    }

    let default_user = args.effective_db_user(env_name);
    let user: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Database user")
        .default(default_user)
        .interact_text()?;

    let password = if let Some(ref pw) = args.db_password {
        pw.clone()
    } else {
        let use_generated = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Generate a secure password automatically?")
            .default(true)
            .interact()?;

        if use_generated {
            let generated = generate_password();
            CliService::success(&format!("Generated password: {}", generated));
            generated
        } else {
            Password::with_theme(&ColorfulTheme::default())
                .with_prompt("Database password")
                .interact()?
        }
    };

    if password.is_empty() {
        anyhow::bail!("Password is required");
    }

    let default_db = args.effective_db_name(env_name);
    let database: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Database name")
        .default(default_db)
        .interact_text()?;

    let config = PostgresConfig {
        host,
        port,
        user,
        password,
        database,
    };

    let can_connect = test_connection(&config).await;

    if can_connect {
        CliService::success("Successfully connected to database!");
        enable_extensions(&config).await?;
    } else {
        CliService::warning("Cannot connect with provided credentials.");
        CliService::info("The database or user may not exist yet.");

        let create_db = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Create database and user now? (requires superuser)")
            .default(true)
            .interact()?;

        if create_db {
            create_database_interactive(&config).await?;
            enable_extensions(&config).await?;
        } else {
            CliService::warning("Skipping database creation. You may need to create it manually.");
        }
    }

    Ok(config)
}

async fn create_database_interactive(config: &PostgresConfig) -> Result<()> {
    CliService::info("Enter PostgreSQL superuser credentials (typically 'postgres'):");

    let superuser: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Superuser name")
        .default("postgres".to_string())
        .interact_text()?;

    let superpass: String = Password::with_theme(&ColorfulTheme::default())
        .with_prompt("Superuser password")
        .interact()?;

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

    let user_exists: bool = sqlx::query("SELECT EXISTS(SELECT 1 FROM pg_roles WHERE rolname = $1)")
        .bind(&config.user)
        .fetch_one(&pool)
        .await?
        .get(0);

    if !user_exists {
        CliService::info(&format!("Creating user '{}'...", config.user));
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
        CliService::info(&format!("Creating database '{}'...", config.database));
        let create_db_sql = format!(
            "CREATE DATABASE \"{}\" OWNER \"{}\"",
            config.database.replace('"', "\"\""),
            config.user.replace('"', "\"\"")
        );
        sqlx::query(&create_db_sql).execute(&pool).await?;
        CliService::success(&format!("Created database '{}'", config.database));
    }

    let grant_sql = format!(
        "GRANT ALL PRIVILEGES ON DATABASE \"{}\" TO \"{}\"",
        config.database.replace('"', "\"\""),
        config.user.replace('"', "\"\"")
    );
    sqlx::query(&grant_sql).execute(&pool).await?;

    pool.close().await;

    CliService::success("Database and user setup complete");
    Ok(())
}
