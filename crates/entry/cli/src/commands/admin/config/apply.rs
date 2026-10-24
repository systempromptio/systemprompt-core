//! `admin config apply`: the explicit code-to-database step the deploy
//! pipeline runs after a release (credential.md §3.1 rule 3).
//!
//! Every registered plane, or the ones named with `--plane`, is merged entity
//! by entity against the base the last apply recorded: what only the file
//! changed is written, what only the console changed is kept and reported as
//! drift, and what both changed is recorded as a conflict for `/admin/sync`
//! and left alone. One applier runs at a time across every replica (an
//! advisory lock), so a rolling deploy cannot race itself. `--dry-run` rolls
//! the whole apply back after computing it. The command exits non-zero when
//! any conflict is open after the run, so a pipeline stops and a person
//! decides.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, bail};
use clap::Args;
use serde::Serialize;
use systemprompt_extension::prelude::ChangeActor;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::repository::ConfigReconcileRepository;
use systemprompt_scheduler::services::{
    ApplyOptions, ConfigApplyService, ConfigReconcileService, MergeReport,
};

use crate::CliConfig;
use crate::shared::{CommandOutput, render_result};

#[derive(Debug, Clone, Args)]
pub struct ApplyArgs {
    #[arg(
        long = "plane",
        help = "Apply only this plane (repeatable); default every plane"
    )]
    pub planes: Vec<String>,

    #[arg(
        long,
        help = "Why the declaration is being applied; recorded in the audit"
    )]
    pub reason: String,

    #[arg(long, help = "Compute the merge and roll it back without writing")]
    pub dry_run: bool,

    #[arg(
        long,
        default_value_t = 120,
        help = "Seconds to wait for another applier to finish"
    )]
    pub wait_secs: u64,
}

#[derive(Debug, Serialize)]
struct PlaneRow {
    plane: String,
    adopted: bool,
    applied: usize,
    converged: usize,
    drift: usize,
    conflicts: usize,
    unchanged: usize,
}

pub async fn execute(args: &ApplyArgs, config: &CliConfig) -> Result<()> {
    if args.reason.trim().is_empty() {
        bail!("--reason must not be empty; it is recorded in config_change_audit");
    }
    let ctx = Arc::new(AppContext::new().await?);
    let report = run(args, &ctx).await?;
    let conflicts = report.conflicts();
    render_result(&render(&report), config);
    if conflicts > 0 && !report.dry_run {
        bail!(
            "{conflicts} entities changed in both the file and the database since the last \
             apply; resolve them on /admin/sync"
        );
    }
    Ok(())
}

pub async fn run(args: &ApplyArgs, ctx: &Arc<AppContext>) -> Result<MergeReport> {
    let service = ConfigApplyService::new(ConfigReconcileService::from_app_context(
        ctx,
        ConfigReconcileRepository::new(ctx.db_pool()),
    )?);
    let options = ApplyOptions {
        planes: &args.planes,
        actor: ChangeActor::Cli,
        reason: args.reason.trim(),
        dry_run: args.dry_run,
        wait: Duration::from_secs(args.wait_secs),
    };
    Ok(service.apply(&options).await?)
}

fn render(report: &MergeReport) -> CommandOutput {
    let rows: Vec<PlaneRow> = report
        .planes
        .iter()
        .filter(|p| !p.undeclared)
        .map(|p| PlaneRow {
            plane: p.plane.clone(),
            adopted: p.adopted,
            applied: p.applied.len(),
            converged: p.converged.len(),
            drift: p.drift.len(),
            conflicts: p.conflicts.len(),
            unchanged: p.unchanged,
        })
        .collect();
    let title = format!(
        "{}config apply: {} entities written, {} conflicts",
        if report.dry_run { "(dry run) " } else { "" },
        report.applied(),
        report.conflicts()
    );
    CommandOutput::table_of(
        vec![
            "plane",
            "adopted",
            "applied",
            "converged",
            "drift",
            "conflicts",
            "unchanged",
        ],
        &rows,
    )
    .with_title(title)
}
