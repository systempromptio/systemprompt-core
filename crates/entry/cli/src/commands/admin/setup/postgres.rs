use anyhow::{anyhow, Context, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input, Password, Select};
use rand::distr::Alphanumeric;
use rand::{rng, Rng};
use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use std::net::ToSocketAddrs;
use std::time::Duration;
use systemprompt_logging::CliService;

use super::SetupArgs;
use crate::CliConfig;

#[derive(Debug, Clone)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: String,
}

impl PostgresConfig {
    pub fn database_url(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.user, self.password, self.host, self.port, self.database
        )
    }
}

pub fn generate_password() -> String {
    let mut rng = rng();
    (0..16)
        .map(|_| rng.sample(Alphanumeric))
        .map(char::from)
        .collect()
}

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

pub fn detect_postgresql(host: &str, port: u16) -> bool {
    let addr = format!("{}:{}", host, port);
    let socket_addrs = match addr.to_socket_addrs() {
        Ok(addrs) => addrs.collect::<Vec<_>>(),
        Err(e) => {
            tracing::debug!(host = %host, port = %port, error = %e, "Failed to resolve socket address");
            return false;
        },
    };

    for socket_addr in socket_addrs {
        if std::net::TcpStream::connect_timeout(&socket_addr, Duration::from_secs(3)).is_ok() {
            return true;
        }
    }

    false
}

pub async fn test_connection(config: &PostgresConfig) -> bool {
    let Ok(pool) = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&config.database_url())
        .await
    else {
        return false;
    };

    let result = sqlx::query("SELECT 1").fetch_one(&pool).await.is_ok();
    pool.close().await;
    result
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

pub async fn enable_extensions(config: &PostgresConfig) -> Result<()> {
    let pool = match PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&config.database_url())
        .await
    {
        Ok(pool) => pool,
        Err(e) => {
            CliService::warning(&format!("Could not enable extensions: {}", e));
            return Ok(());
        },
    };

    let extensions = ["uuid-ossp", "unaccent", "pg_trgm"];

    for ext in extensions {
        let sql = format!("CREATE EXTENSION IF NOT EXISTS \"{}\"", ext);
        if let Err(e) = sqlx::query(&sql).execute(&pool).await {
            CliService::warning(&format!("Could not create extension '{}': {}", ext, e));
        }
    }

    pool.close().await;
    Ok(())
}
