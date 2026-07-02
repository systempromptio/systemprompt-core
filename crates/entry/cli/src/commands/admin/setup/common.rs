use anyhow::Result;
use rand::distr::Alphanumeric;
use rand::{RngExt, rng};
use sqlx::postgres::PgPoolOptions;
use std::net::ToSocketAddrs;
use std::time::Duration;
use systemprompt_logging::CliService;

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

    let result = sqlx::query_scalar!("SELECT 1 as one")
        .fetch_one(&pool)
        .await
        .is_ok();
    pool.close().await;
    result
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
        if let Err(e) = sqlx::query(sqlx::AssertSqlSafe(sql)).execute(&pool).await {
            CliService::warning(&format!("Could not create extension '{}': {}", ext, e));
        }
    }

    pool.close().await;
    Ok(())
}
