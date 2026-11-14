//! Shared setup types: Postgres connection configuration.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow};
use rand::distr::Alphanumeric;
use rand::{RngExt, rng};
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::net::ToSocketAddrs;
use std::time::Duration;
use url::Url;

const REQUIRED_EXTENSIONS: [&str; 3] = ["uuid-ossp", "unaccent", "pg_trgm"];

#[derive(Debug, Clone)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: String,
}

impl PostgresConfig {
    pub fn database_url(&self) -> Result<String> {
        postgres_url(
            &self.user,
            &self.password,
            &self.host,
            self.port,
            &self.database,
        )
    }

    #[must_use]
    pub fn connect_options(&self) -> PgConnectOptions {
        connect_options(
            &self.user,
            &self.password,
            &self.host,
            self.port,
            &self.database,
        )
    }

    #[must_use]
    pub fn superuser_options(&self, superuser: &str, password: &str) -> PgConnectOptions {
        connect_options(superuser, password, &self.host, self.port, "postgres")
    }
}

pub fn postgres_url(
    user: &str,
    password: &str,
    host: &str,
    port: u16,
    database: &str,
) -> Result<String> {
    let mut url = Url::parse("postgres://localhost").context("Invalid base Postgres URL")?;
    url.set_host(Some(host))
        .with_context(|| format!("Invalid PostgreSQL host '{host}'"))?;
    url.set_port(Some(port))
        .map_err(|()| anyhow!("PostgreSQL URL cannot carry a port"))?;
    url.set_username(user)
        .map_err(|()| anyhow!("PostgreSQL URL cannot carry a user name"))?;
    url.set_password(Some(password))
        .map_err(|()| anyhow!("PostgreSQL URL cannot carry a password"))?;
    url.path_segments_mut()
        .map_err(|()| anyhow!("PostgreSQL URL cannot carry a database name"))?
        .push(database);
    Ok(url.into())
}

fn connect_options(
    user: &str,
    password: &str,
    host: &str,
    port: u16,
    database: &str,
) -> PgConnectOptions {
    PgConnectOptions::new()
        .host(host)
        .port(port)
        .username(user)
        .password(password)
        .database(database)
}

pub async fn connect(options: PgConnectOptions) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(options)
        .await
}

pub fn generate_password() -> String {
    let mut rng = rng();
    (0..16)
        .map(|_| rng.sample(Alphanumeric))
        .map(char::from)
        .collect()
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
    let Ok(pool) = connect(config.connect_options()).await else {
        return false;
    };

    let result = sqlx::query_scalar!("SELECT 1 as one")
        .fetch_one(&pool)
        .await
        .is_ok();
    pool.close().await;
    result
}

// Why: PostgreSQL does not serialise concurrent `CREATE EXTENSION IF NOT
// EXISTS` for one name; the loser fails on the catalog's unique index although
// the extension is then installed.
async fn extension_installed(pool: &PgPool, name: &str) -> bool {
    let installed = sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM pg_extension WHERE extname = $1) as "installed!""#,
        name
    )
    .fetch_one(pool)
    .await;
    matches!(installed, Ok(true))
}

pub async fn enable_extensions(config: &PostgresConfig) -> Result<()> {
    let pool = connect(config.connect_options()).await.with_context(|| {
        format!(
            "Could not connect to database '{}' to enable extensions",
            config.database
        )
    })?;

    let mut failed = Vec::new();
    for ext in REQUIRED_EXTENSIONS {
        let sql = format!(
            "CREATE EXTENSION IF NOT EXISTS {}",
            systemprompt_database::quote_identifier(ext)
        );
        if let Err(error) = sqlx::query(sqlx::AssertSqlSafe(sql)).execute(&pool).await {
            if extension_installed(&pool, ext).await {
                continue;
            }
            tracing::warn!(extension = ext, error = %error, "CREATE EXTENSION failed");
            failed.push(format!("{ext} ({error})"));
        }
    }
    pool.close().await;

    if failed.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(
            "Required PostgreSQL extensions could not be created: {}. Create them as a superuser \
             in database '{}' and re-run setup.",
            failed.join(", "),
            config.database
        ))
    }
}
