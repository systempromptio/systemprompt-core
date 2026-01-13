use anyhow::{Context, Result};
use std::{env, sync::Arc};
use systemprompt::identifiers::McpServerId;
use systemprompt::models::{Config, ProfileBootstrap, SecretsBootstrap};
use systemprompt::system::AppContext;
use systemprompt_admin::AdminServer;
use tokio::net::TcpListener;

/// Default service ID - MUST match the key in `mcp_servers` config
const DEFAULT_SERVICE_ID: &str = "systemprompt-admin";
const DEFAULT_PORT: u16 = 5002;

#[tokio::main]
async fn main() -> Result<()> {
    ProfileBootstrap::init().context("Failed to initialize profile")?;
    SecretsBootstrap::init().context("Failed to initialize secrets")?;
    Config::init().context("Failed to initialize configuration")?;

    let ctx = Arc::new(
        AppContext::new()
            .await
            .context("Failed to initialize application context")?,
    );

    // Initialize logging with database persistence
    systemprompt::logging::init_logging(ctx.db_pool().clone());

    let service_id = McpServerId::from_env().unwrap_or_else(|_| {
        tracing::warn!("MCP_SERVICE_ID not set, using default: {DEFAULT_SERVICE_ID}");
        McpServerId::new(DEFAULT_SERVICE_ID)
    });

    let port = env::var("MCP_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or_else(|| {
            tracing::warn!("MCP_PORT not set, using default: {DEFAULT_PORT}");
            DEFAULT_PORT
        });

    let server = AdminServer::new(ctx.db_pool().clone(), service_id.clone(), ctx.clone()).await;
    let router = systemprompt::mcp::create_router(server, &ctx);
    let addr = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(&addr).await?;

    tracing::info!(service_id = %service_id, addr = %addr, "Admin MCP server listening");

    axum::serve(listener, router).await?;

    Ok(())
}
