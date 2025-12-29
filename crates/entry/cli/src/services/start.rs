use anyhow::Result;
use std::time::Instant;
use systemprompt_cloud::CredentialsBootstrap;
use systemprompt_models::ProfileBootstrap;
use systemprompt_traits::{startup_channel, Phase, StartupEvent, StartupEventExt};

use crate::presentation::StartupRenderer;

pub async fn execute(
    all: bool,
    api: bool,
    agents: bool,
    mcp: bool,
    _foreground: bool,
    skip_web: bool,
    skip_migrate: bool,
) -> Result<()> {
    let start_time = Instant::now();
    let start_all = all || (!api && !agents && !mcp);

    let (tx, rx) = startup_channel();

    let renderer = StartupRenderer::new(rx);
    let render_handle = tokio::spawn(renderer.run());

    let result = run_startup(start_all, api, agents, mcp, skip_web, skip_migrate, &tx).await;

    if let Err(e) = &result {
        let _ = tx.send(StartupEvent::StartupFailed {
            error: e.to_string(),
            duration: start_time.elapsed(),
        });
    }

    drop(tx);
    let _ = render_handle.await;

    result.map(|_| ())
}

async fn run_startup(
    start_all: bool,
    api: bool,
    agents: bool,
    mcp: bool,
    skip_web: bool,
    skip_migrate: bool,
    events: &systemprompt_traits::StartupEventSender,
) -> Result<String> {
    events.phase_started(Phase::PreFlight);

    if let Ok(profile) = ProfileBootstrap::get() {
        if let Some(cloud) = &profile.cloud {
            if cloud.enabled {
                match CredentialsBootstrap::get() {
                    Ok(Some(_)) => {
                        events.info("Cloud features enabled with valid credentials");
                    },
                    Ok(None) | Err(_) => {
                        events.warning(
                            "Cloud features enabled but no credentials. Run 'systemprompt cloud \
                             login'",
                        );
                    },
                }
            }
        }
    }

    if !skip_web {
        crate::common::web::build_web_assets().await?;
    }

    events.phase_completed(Phase::PreFlight);

    if !skip_migrate {
        events.phase_started(Phase::Database);
        super::db::execute(super::db::DbCommands::Migrate).await?;
        events.phase_completed(Phase::Database);
    }

    if start_all || api {
        let api_url = super::serve::execute_with_events(true, Some(events)).await?;
        return Ok(api_url);
    }

    if agents {
        events.phase_started(Phase::Agents);
        events.info("Agents start automatically with the API server");
        events.phase_completed(Phase::Agents);
    }

    if mcp {
        events.phase_started(Phase::McpServers);
        events.info("MCP servers start automatically with the API server");
        events.phase_completed(Phase::McpServers);
    }

    Ok("http://127.0.0.1:8080".to_string())
}
