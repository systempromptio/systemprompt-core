use anyhow::{Context, Result};
use clap::Subcommand;
use std::env;
use std::sync::Arc;
use systemprompt_core_agent::services::a2a_server::Server;
use systemprompt_core_agent::services::agent_orchestration::AgentOrchestrator;
use systemprompt_core_ai::AiService;
use systemprompt_core_logging::CliService;
use systemprompt_core_mcp::McpToolProvider;
use systemprompt_loader::ConfigLoader;
use systemprompt_runtime::AppContext;
use tokio::signal;
use tracing;

#[derive(Subcommand)]
pub enum AgentCommands {
    #[command(about = "Enable and auto-start one or more A2A agents")]
    Enable {
        agent_name: Option<String>,
        #[arg(long)]
        all: bool,
    },
    #[command(about = "Disable and stop A2A agents")]
    Disable {
        agent_name: Option<String>,
        #[arg(long)]
        all: bool,
    },
    #[command(about = "Restart a specific A2A agent")]
    Restart { agent_name: String },
    #[command(about = "Show detailed status of all registered A2A agents")]
    Status,
    #[command(about = "List all registered agents with their current state")]
    List,
    #[command(about = "Validate agent configuration and connectivity")]
    Validate {
        agent_name: Option<String>,
        #[arg(long)]
        all: bool,
    },
    #[command(about = "Perform health checks on running agents")]
    Health {
        agent_name: Option<String>,
        #[arg(long)]
        all: bool,
    },
    #[command(about = "Run orchestrator in daemon mode")]
    Daemon {
        #[arg(long, default_value = "30")]
        health_interval: u64,
    },
    #[command(about = "Clean up orphaned processes")]
    Cleanup,
    #[command(about = "Delete agents from the system")]
    Delete {
        agent_name: Option<String>,
        #[arg(long)]
        all: bool,
    },
    #[command(about = "Run agent A2A server")]
    Run {
        #[arg(long, env = "AGENT_NAME")]
        agent_name: String,
        #[arg(long, env = "AGENT_PORT")]
        port: u16,
    },
}

enum TargetSelection {
    All,
    Single(String),
}

fn require_target(agent_name: Option<String>, all: bool) -> Result<TargetSelection> {
    match (all, agent_name) {
        (true, _) => Ok(TargetSelection::All),
        (false, Some(name)) => Ok(TargetSelection::Single(name)),
        (false, None) => Err(anyhow::anyhow!("Please specify agent name or use --all")),
    }
}

pub async fn execute(cmd: AgentCommands, ctx: Arc<AppContext>) -> Result<()> {
    env::set_var("SYSTEMPROMPT_NON_INTERACTIVE", "1");

    let orchestrator = AgentOrchestrator::new(Arc::clone(&ctx), None)
        .await
        .context("Failed to initialize agent orchestrator")?;

    match cmd {
        AgentCommands::Enable { agent_name, all } => {
            handle_enable(&orchestrator, agent_name, all).await
        },
        AgentCommands::Disable { agent_name, all } => {
            handle_disable(&orchestrator, agent_name, all).await
        },
        AgentCommands::Restart { agent_name } => handle_restart(&orchestrator, agent_name).await,
        AgentCommands::Status | AgentCommands::List => handle_status(&orchestrator).await,
        AgentCommands::Validate { agent_name, all } => {
            handle_validate(&orchestrator, agent_name, all).await
        },
        AgentCommands::Health { agent_name, all } => {
            handle_health(&orchestrator, agent_name, all).await
        },
        AgentCommands::Daemon { health_interval: _ } => handle_daemon(orchestrator).await,
        AgentCommands::Cleanup => handle_cleanup(&orchestrator).await,
        AgentCommands::Delete { agent_name, all } => {
            handle_delete(&orchestrator, agent_name, all).await
        },
        AgentCommands::Run { agent_name, port } => {
            run_agent_server(&ctx, &orchestrator, agent_name, port).await
        },
    }
}

async fn handle_enable(
    orchestrator: &AgentOrchestrator,
    agent_name: Option<String>,
    all: bool,
) -> Result<()> {
    match require_target(agent_name, all)? {
        TargetSelection::All => {
            let service_ids = orchestrator.start_all(None).await?;
            CliService::success(&format!("Enabled {} agents", service_ids.len()));
        },
        TargetSelection::Single(name) => {
            let service_id = orchestrator.enable_agent(&name, None).await?;
            CliService::success(&format!("Agent enabled with service ID: {}", service_id));
        },
    }
    Ok(())
}

async fn handle_disable(
    orchestrator: &AgentOrchestrator,
    agent_name: Option<String>,
    all: bool,
) -> Result<()> {
    match require_target(agent_name, all)? {
        TargetSelection::All => {
            orchestrator.disable_all().await?;
            CliService::success("All agents disabled");
        },
        TargetSelection::Single(name) => {
            orchestrator.disable_agent(&name).await?;
            CliService::success(&format!("Agent {} disabled", name));
        },
    }
    Ok(())
}

async fn handle_restart(orchestrator: &AgentOrchestrator, agent_name: String) -> Result<()> {
    let service_id = orchestrator.restart_agent(&agent_name, None).await?;
    CliService::success(&format!(
        "Agent {} restarted with service ID: {}",
        agent_name, service_id
    ));
    Ok(())
}

async fn handle_status(orchestrator: &AgentOrchestrator) -> Result<()> {
    let all_agents = orchestrator.list_all().await?;
    for (agent_id, status) in all_agents {
        CliService::info(&format!("{}: {:?}", agent_id, status));
    }
    Ok(())
}

async fn handle_validate(
    orchestrator: &AgentOrchestrator,
    agent_name: Option<String>,
    all: bool,
) -> Result<()> {
    match require_target(agent_name, all)? {
        TargetSelection::All => {
            let all_agents = orchestrator.list_all().await?;
            CliService::info(&format!("Validating {} agents...", all_agents.len()));

            for (agent_name, _) in all_agents {
                let report = orchestrator.validate_agent(&agent_name).await?;
                if !report.valid {
                    CliService::error(&format!("{}: {}", agent_name, report.issues.join(", ")));
                }
            }
        },
        TargetSelection::Single(name) => {
            orchestrator.validate_agent(&name).await?;
        },
    }
    Ok(())
}

async fn handle_health(
    orchestrator: &AgentOrchestrator,
    agent_name: Option<String>,
    all: bool,
) -> Result<()> {
    match require_target(agent_name, all)? {
        TargetSelection::All => {
            let reports = orchestrator.health_check_all().await?;
            CliService::success(&format!(
                "Health check completed for {} agents",
                reports.len()
            ));
        },
        TargetSelection::Single(name) => {
            let result = orchestrator.health_check(&name).await?;
            let message = format!(
                "{}: {} ({}ms)",
                name, result.message, result.response_time_ms
            );
            if result.healthy {
                CliService::success(&message);
            } else {
                CliService::error(&message);
            }
        },
    }
    Ok(())
}

async fn handle_daemon(mut orchestrator: AgentOrchestrator) -> Result<()> {
    orchestrator.run_daemon().await?;
    Ok(())
}

async fn handle_cleanup(orchestrator: &AgentOrchestrator) -> Result<()> {
    orchestrator.cleanup_orphaned_processes().await?;
    Ok(())
}

async fn handle_delete(
    orchestrator: &AgentOrchestrator,
    agent_name: Option<String>,
    all: bool,
) -> Result<()> {
    match require_target(agent_name, all)? {
        TargetSelection::All => {
            let deleted_count = orchestrator.delete_all_agents().await?;
            CliService::success(&format!("Deleted {} agents", deleted_count));
        },
        TargetSelection::Single(name) => {
            orchestrator.delete_agent(&name).await?;
            CliService::success(&format!("Agent {} deleted", name));
        },
    }
    Ok(())
}

#[tracing::instrument(name = "agent_server", skip(ctx, orchestrator))]
async fn run_agent_server(
    ctx: &Arc<AppContext>,
    orchestrator: &AgentOrchestrator,
    name: String,
    port: u16,
) -> Result<()> {
    let db_pool = Arc::clone(ctx.db_pool());

    systemprompt_core_logging::init_logging(Arc::clone(&db_pool));

    let pid = std::process::id();
    orchestrator
        .update_agent_running(&name, pid, port)
        .await
        .context("Failed to update agent status to running")?;

    let services_config = ConfigLoader::load().context("Failed to load services config")?;
    let tool_provider = Arc::new(McpToolProvider::new(ctx));
    let ai_service = Arc::new(
        AiService::new(ctx, &services_config.ai, tool_provider)
            .context("Failed to create AI service")?,
    );

    let server = match Server::new(
        Arc::clone(&db_pool),
        Arc::clone(ctx),
        ai_service,
        Some(name.clone()),
        port,
    )
    .await
    {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "Failed to create A2A server");
            return Err(e.context("Failed to create A2A server"));
        },
    };

    let shutdown_ctx = Arc::clone(ctx);
    let shutdown_agent_name = name.clone();
    let shutdown = async move {
        signal::ctrl_c().await.ok();
        if let Ok(shutdown_orchestrator) = AgentOrchestrator::new(shutdown_ctx, None).await {
            let _ = shutdown_orchestrator
                .update_agent_stopped(&shutdown_agent_name)
                .await;
        }
    };

    server
        .run_with_shutdown(shutdown)
        .await
        .context("Server failed during execution")?;

    tracing::info!("Server shutdown completed");
    Ok(())
}
