//! `marketplace` CLI command group: manifest-assembly diagnostics.
//!
//! [`MarketplaceCommands::Explain`] dry-runs the same assembly the bridge
//! manifest endpoint performs and reports, per catalogue entry, whether it is
//! delivered and — when it is not — the stage that dropped it and why.
//! [`MarketplaceCommands::Import`] converts an Anthropic-format authoring
//! repository into the services tree the loader discovers.
//!
//! `explain` reads the services tree and the published API URL through the
//! services-tree grant and the loaded services configuration from the open
//! `ServicesState` seed; it needs no database. `import` reads and writes
//! only the paths it is given.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod import;

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use serde::Serialize;
use systemprompt_extension::cli::{
    CliCommandGroup, CommandDescriptor, CommandError, CommandHost, CommandOutput, DataImpact,
    DescribeCommand,
};
use systemprompt_identifiers::UserId;
use systemprompt_marketplace::{
    AllowAllFilter, AssembleRequest, ManifestService, ManifestTrace, MarketplaceCache, TraceKind,
};

#[derive(Debug, Subcommand)]
pub enum MarketplaceCommands {
    #[command(about = "Explain which catalogue entries reach the bridge manifest and why")]
    Explain(ExplainArgs),

    #[command(about = "Import an Anthropic-format authoring tree into a services tree")]
    Import(import::ImportArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ExplainArgs {
    #[arg(long, help = "Only report this skill id")]
    pub skill: Option<String>,

    #[arg(long, help = "Only report this plugin id")]
    pub plugin: Option<String>,

    #[arg(long, help = "User id to assemble for (extension filters may use it)")]
    pub user: Option<String>,
}

#[derive(Debug, Serialize)]
struct ExplainRow {
    kind: TraceKind,
    id: String,
    delivered: bool,
    dropped_at: String,
    reason: String,
}

pub async fn execute(
    command: MarketplaceCommands,
    host: &dyn CommandHost,
) -> Result<CommandOutput> {
    match command {
        MarketplaceCommands::Explain(args) => explain(&args, host)
            .await
            .context("Failed to explain manifest"),
        MarketplaceCommands::Import(args) => import::execute(&args),
    }
}

pub async fn explain(args: &ExplainArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let tree = host.services_tree()?;
    let services = super::services_config(host)
        .await
        .context("Failed to load services config")?;
    let user_id = UserId::new(
        args.user
            .clone()
            .unwrap_or_else(|| "cli-explain".to_owned()),
    );

    let mut trace = ManifestTrace::default();
    let candidate = ManifestService::assemble_candidate_traced(
        &AssembleRequest {
            services: services.as_ref(),
            services_root: tree.root(),
            filter: &AllowAllFilter,
            user_id: &user_id,
            cache: &MarketplaceCache::default(),
        },
        tree.api_external_url(),
        &mut trace,
    )
    .await
    .context("Manifest assembly failed")?;

    let mut rows = build_rows(&trace, &candidate);

    if let Some(skill) = &args.skill {
        rows.retain(|r| r.kind == TraceKind::Skill && &r.id == skill);
    }
    if let Some(plugin) = &args.plugin {
        rows.retain(|r| r.kind == TraceKind::Plugin && &r.id == plugin);
    }
    rows.sort_by(|a, b| (a.kind.to_string(), &a.id).cmp(&(b.kind.to_string(), &b.id)));

    Ok(CommandOutput::table_of(
        vec!["kind", "id", "delivered", "dropped_at", "reason"],
        &rows,
    )
    .with_title("Manifest Assembly Explain"))
}

impl MarketplaceCommands {
    pub const fn data_impact(&self) -> DataImpact {
        match self {
            Self::Explain(_) | Self::Import(_) => DataImpact::Preserving,
        }
    }
}

impl DescribeCommand for MarketplaceCommands {
    fn descriptor(&self) -> CommandDescriptor {
        match self {
            Self::Explain(_) => CommandDescriptor::FULL,
            Self::Import(_) => CommandDescriptor::NONE,
        }
        .with_data_impact(self.data_impact())
    }
}

impl CliCommandGroup for MarketplaceCommands {
    const NAME: &'static str = "marketplace";
    const ABOUT: &'static str = "Marketplace manifest diagnostics";

    async fn run(self, host: &dyn CommandHost) -> Result<CommandOutput, CommandError> {
        execute(self, host).await.map_err(super::command_error)
    }
}

fn build_rows(
    trace: &ManifestTrace,
    candidate: &systemprompt_marketplace::MarketplaceCandidate,
) -> Vec<ExplainRow> {
    let mut rows: Vec<ExplainRow> = trace
        .events
        .iter()
        .map(|event| ExplainRow {
            kind: event.kind,
            id: event.id.clone(),
            delivered: false,
            dropped_at: event.stage.to_string(),
            reason: event.reason.clone(),
        })
        .collect();
    let delivered = |kind: TraceKind, id: String| ExplainRow {
        kind,
        id,
        delivered: true,
        dropped_at: String::new(),
        reason: String::new(),
    };
    rows.extend(
        candidate
            .skills
            .iter()
            .map(|s| delivered(TraceKind::Skill, s.id.as_str().to_owned())),
    );
    rows.extend(
        candidate
            .plugins
            .iter()
            .map(|p| delivered(TraceKind::Plugin, p.id.as_str().to_owned())),
    );
    rows.extend(
        candidate
            .agents
            .iter()
            .map(|a| delivered(TraceKind::Agent, a.id.as_str().to_owned())),
    );
    rows.extend(
        candidate
            .managed_mcp_servers
            .iter()
            .map(|m| delivered(TraceKind::McpServer, m.name.as_str().to_owned())),
    );
    rows.extend(
        candidate
            .artifacts
            .iter()
            .map(|a| delivered(TraceKind::Artifact, a.id.as_str().to_owned())),
    );
    rows
}
