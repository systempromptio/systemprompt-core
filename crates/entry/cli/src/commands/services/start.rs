use crate::cli_settings::CliConfig;
use crate::presentation::StartupRenderer;
use anyhow::Result;
use std::time::Instant;
use systemprompt_cloud::CredentialsBootstrap;
use systemprompt_models::ProfileBootstrap;
use systemprompt_traits::{startup_channel, Phase, StartupEvent, StartupEventExt};

pub struct ServiceTarget {
    pub api: bool,
    pub agents: bool,
    pub mcp: bool,
}

#[derive(Clone, Copy)]
pub struct ServiceFlags {
    pub all: bool,
    pub targets: ServiceTargetFlags,
}

#[derive(Clone, Copy)]
pub struct ServiceTargetFlags {
    pub api: bool,
    pub agents: bool,
    pub mcp: bool,
}

impl ServiceTarget {
    pub const fn all() -> Self {
        Self {
            api: true,
            agents: true,
            mcp: true,
        }
    }

    pub const fn from_flags(flags: ServiceFlags) -> Self {
        if flags.all || (!flags.targets.api && !flags.targets.agents && !flags.targets.mcp) {
            Self::all()
        } else {
            Self {
                api: flags.targets.api,
                agents: flags.targets.agents,
                mcp: flags.targets.mcp,
            }
        }
    }
}

pub struct StartupOptions {
    pub skip_web: bool,
    pub skip_migrate: bool,
}

pub async fn execute(
    target: ServiceTarget,
    options: StartupOptions,
    config: &CliConfig,
) -> Result<()> {
    let start_time = Instant::now();

    let (tx, rx) = startup_channel();

    let renderer = StartupRenderer::new(rx);
    let render_handle = tokio::spawn(renderer.run());

    let result = run_startup(&target, &options, config, &tx).await;

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
    target: &ServiceTarget,
    options: &StartupOptions,
    config: &CliConfig,
    events: &systemprompt_traits::StartupEventSender,
) -> Result<String> {
    events.phase_started(Phase::PreFlight);

    if let Ok(profile) = ProfileBootstrap::get() {
        if profile.cloud.is_some() {
            match CredentialsBootstrap::get() {
                Ok(Some(_)) => {
                    events.info("Cloud features enabled with valid credentials");
                },
                Ok(None) | Err(_) => {
                    events.warning("Cloud credentials not found. Run 'systemprompt cloud login'");
                },
            }
        }
    }

    if !options.skip_web {
        crate::shared::web::build_web_assets()?;
    }

    events.phase_completed(Phase::PreFlight);

    if !options.skip_migrate {
        events.phase_started(Phase::Database);
        super::db::execute(super::db::DbCommands::Migrate, config).await?;
        events.phase_completed(Phase::Database);
    }

    if target.api {
        let api_url = super::serve::execute_with_events(true, false, config, Some(events)).await?;
        return Ok(api_url);
    }

    if target.agents {
        events.phase_started(Phase::Agents);
        events.info("Agents start automatically with the API server");
        events.phase_completed(Phase::Agents);
    }

    if target.mcp {
        events.phase_started(Phase::McpServers);
        events.info("MCP servers start automatically with the API server");
        events.phase_completed(Phase::McpServers);
    }

    Ok("http://127.0.0.1:8080".to_string())
}
