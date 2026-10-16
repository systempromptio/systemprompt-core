//! `admin config planes`: list, diff and apply declared configuration planes.
//!
//! Every subcommand runs headless against the active profile's database; no
//! browser session is involved. `apply` writes through the same audited path
//! as the `config_reconcile` job, recording each changed entity in
//! `config_change_audit` with actor `cli` and the given reason.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Result, bail};
use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;
use systemprompt_extension::prelude::{ApplyMode, ChangeActor, ChangeRequest};
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::services::ConfigReconcileService;

use crate::CliConfig;
use crate::shared::{CommandOutput, render_result};

#[derive(Debug, Subcommand)]
pub enum PlanesCommands {
    #[command(about = "List registered config planes with authority, hash and last reconcile")]
    List,

    #[command(about = "Show the drift between a plane's declaration and the database")]
    Diff(DiffArgs),

    #[command(about = "Apply a plane's declaration to the database (audited)")]
    Apply(ApplyArgs),
}

#[derive(Debug, Clone, Args)]
pub struct DiffArgs {
    #[arg(long)]
    pub plane: String,
}

#[derive(Debug, Clone, Args)]
pub struct ApplyArgs {
    #[arg(long)]
    pub plane: String,

    #[arg(
        long,
        help = "Why the declaration is being applied; recorded in the audit"
    )]
    pub reason: String,

    #[arg(long, value_enum, default_value_t = ModeArg::Overwrite)]
    pub mode: ModeArg,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ModeArg {
    Overwrite,
    InsertOnly,
}

impl From<ModeArg> for ApplyMode {
    fn from(mode: ModeArg) -> Self {
        match mode {
            ModeArg::Overwrite => Self::Overwrite,
            ModeArg::InsertOnly => Self::InsertOnly,
        }
    }
}

#[derive(Debug, Serialize)]
struct PlaneListRow {
    plane: &'static str,
    declared_path: &'static str,
    authority: &'static str,
    hash: Option<String>,
    drift_rows: Option<i32>,
    outcome: Option<String>,
    reconciled_at: Option<String>,
}

pub async fn execute(command: PlanesCommands, config: &CliConfig) -> Result<()> {
    let ctx = Arc::new(AppContext::new().await?);
    let service = ConfigReconcileService::from_app_context(
        &ctx,
        systemprompt_scheduler::repository::ConfigReconcileRepository::new(ctx.db_pool()),
    )?;
    let output = match command {
        PlanesCommands::List => list(&service).await?,
        PlanesCommands::Diff(args) => diff(&service, &args.plane).await?,
        PlanesCommands::Apply(args) => apply(&service, &args).await?,
    };
    render_result(&output, config);
    Ok(())
}

async fn list(service: &ConfigReconcileService) -> Result<CommandOutput> {
    let mut states: HashMap<String, _> = service
        .repository()
        .list_states()
        .await?
        .into_iter()
        .map(|row| (row.plane.clone(), row))
        .collect();
    let set = service.planes();
    let mut rows = Vec::new();
    for plane in set.planes() {
        let state = states.remove(plane.name());
        let hash = match plane.load_declared(set.services_root()) {
            Ok(declared) => declared.map(|d| d.hash),
            Err(error) => Some(format!("unreadable: {error}")),
        };
        rows.push(PlaneListRow {
            plane: plane.name(),
            declared_path: plane.declared_path(),
            authority: set.authority_of(plane.as_ref()).as_str(),
            hash,
            drift_rows: state.as_ref().map(|s| s.drift_rows),
            outcome: state.as_ref().map(|s| s.outcome.clone()),
            reconciled_at: state.map(|s| s.reconciled_at.to_rfc3339()),
        });
    }
    Ok(CommandOutput::table_of(
        vec![
            "plane",
            "declared_path",
            "authority",
            "hash",
            "drift_rows",
            "outcome",
            "reconciled_at",
        ],
        &rows,
    )
    .with_title("Config planes"))
}

async fn diff(service: &ConfigReconcileService, plane: &str) -> Result<CommandOutput> {
    let diff = service.diff(plane).await?;
    let title = format!(
        "{} ({}, {}): {} drift rows",
        diff.plane,
        diff.declared_path,
        diff.authority.as_str(),
        diff.rows.len()
    );
    Ok(
        CommandOutput::table_of(vec!["entity", "kind", "declared", "current"], &diff.rows)
            .with_title(title),
    )
}

async fn apply(service: &ConfigReconcileService, args: &ApplyArgs) -> Result<CommandOutput> {
    if args.reason.trim().is_empty() {
        bail!("--reason must not be empty; it is recorded in config_change_audit");
    }
    let change = ChangeRequest {
        actor: ChangeActor::Cli,
        reason: args.reason.trim(),
    };
    let applied = service
        .apply(&args.plane, args.mode.into(), &change)
        .await?;
    Ok(CommandOutput::card_value(
        format!(
            "Applied {} ({} entities changed)",
            applied.plane, applied.changes
        ),
        &applied,
    ))
}
