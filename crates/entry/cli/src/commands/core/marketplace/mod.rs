//! `marketplace` CLI command group: manifest-assembly diagnostics.
//!
//! [`MarketplaceCommands::Explain`] dry-runs the same assembly the bridge
//! manifest endpoint performs and reports, per catalogue entry, whether it is
//! delivered and — when it is not — the stage that dropped it and why.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use serde::Serialize;
use systemprompt_identifiers::UserId;
use systemprompt_marketplace::{AllowAllFilter, ManifestService, ManifestTrace};

use crate::context::CommandContext;
use crate::shared::{CommandOutput, render_result};

#[derive(Debug, Subcommand)]
pub enum MarketplaceCommands {
    #[command(about = "Explain which catalogue entries reach the bridge manifest and why")]
    Explain(ExplainArgs),
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
    kind: String,
    id: String,
    delivered: bool,
    dropped_at: String,
    reason: String,
}

pub async fn execute(command: MarketplaceCommands, ctx: &CommandContext) -> Result<()> {
    match command {
        MarketplaceCommands::Explain(args) => {
            let result = explain(&args).await.context("Failed to explain manifest")?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
    }
}

// Why: public so the assembly can be asserted on directly. `execute` only
// renders, and rendering goes to stdout where a test cannot see it — the same
// split `plugins::validate::execute` uses.
pub async fn explain(args: &ExplainArgs) -> Result<CommandOutput> {
    let profile = systemprompt_config::ProfileBootstrap::get().context("Failed to get profile")?;
    let services =
        systemprompt_loader::ConfigLoader::load().context("Failed to load services config")?;
    let services_root = std::path::PathBuf::from(profile.paths.services.clone());
    let user_id = UserId::new(
        args.user
            .clone()
            .unwrap_or_else(|| "cli-explain".to_owned()),
    );

    let mut trace = ManifestTrace::default();
    let candidate = ManifestService::assemble_candidate_traced(
        &services,
        &services_root,
        &profile.server.api_external_url,
        &AllowAllFilter,
        &user_id,
        &mut trace,
    )
    .await
    .context("Manifest assembly failed")?;

    let mut rows = build_rows(&trace, &candidate);

    if let Some(skill) = &args.skill {
        rows.retain(|r| r.kind == "skill" && &r.id == skill);
    }
    if let Some(plugin) = &args.plugin {
        rows.retain(|r| r.kind == "plugin" && &r.id == plugin);
    }
    rows.sort_by(|a, b| (&a.kind, &a.id).cmp(&(&b.kind, &b.id)));

    Ok(CommandOutput::table_of(
        vec!["kind", "id", "delivered", "dropped_at", "reason"],
        &rows,
    )
    .with_title("Manifest Assembly Explain"))
}

fn build_rows(
    trace: &ManifestTrace,
    candidate: &systemprompt_marketplace::MarketplaceCandidate,
) -> Vec<ExplainRow> {
    let mut rows: Vec<ExplainRow> = trace
        .events
        .iter()
        .map(|event| ExplainRow {
            kind: event.kind.to_string(),
            id: event.id.clone(),
            delivered: false,
            dropped_at: event.stage.to_string(),
            reason: event.reason.clone(),
        })
        .collect();
    let delivered = |kind: &str, id: String| ExplainRow {
        kind: kind.to_owned(),
        id,
        delivered: true,
        dropped_at: String::new(),
        reason: String::new(),
    };
    rows.extend(
        candidate
            .skills
            .iter()
            .map(|s| delivered("skill", s.id.as_str().to_owned())),
    );
    rows.extend(
        candidate
            .plugins
            .iter()
            .map(|p| delivered("plugin", p.id.as_str().to_owned())),
    );
    rows.extend(
        candidate
            .agents
            .iter()
            .map(|a| delivered("agent", a.id.as_str().to_owned())),
    );
    rows.extend(
        candidate
            .managed_mcp_servers
            .iter()
            .map(|m| delivered("mcp-server", m.name.as_str().to_owned())),
    );
    rows.extend(
        candidate
            .artifacts
            .iter()
            .map(|a| delivered("artifact", a.id.as_str().to_owned())),
    );
    rows
}
