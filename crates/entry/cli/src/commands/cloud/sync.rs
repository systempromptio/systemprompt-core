//! `cloud sync export`: the developer's database-to-code step
//! (credential.md §3.1 rule 4).
//!
//! For every plane whose database rows moved since the last apply (console
//! drift and open conflicts), the export renders the plane's declaration with
//! the database side folded in. With `--out <dir>` each file is written at its
//! services-tree path under that directory, ready to copy over `services/`
//! and commit; without it the files are printed. The running system never
//! writes source: this reads the database and nothing else. When the exported
//! change is deployed and applied, file and database agree and the drift
//! clears.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use systemprompt_scheduler::repository::ConfigReconcileRepository;
use systemprompt_scheduler::services::{ConfigApplyService, ConfigReconcileService, PlanePatch};

use crate::context::CommandContext;
use crate::shared::{CommandOutput, render_result};

#[derive(Debug, Subcommand)]
pub enum SyncCommands {
    #[command(about = "Export database changes no apply has taken back as declaration files")]
    Export(ExportArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ExportArgs {
    #[arg(
        long = "plane",
        help = "Export only this plane (repeatable); default every plane"
    )]
    pub planes: Vec<String>,

    #[arg(
        long,
        help = "Write each declaration under this directory at its services-tree path"
    )]
    pub out: Option<PathBuf>,

    #[arg(
        long,
        requires = "out",
        help = "Replace files that already exist under --out"
    )]
    pub force: bool,
}

pub async fn execute(command: SyncCommands, ctx: &CommandContext) -> Result<()> {
    let SyncCommands::Export(args) = command;
    let app = ctx.app_context().await?;
    let service = ConfigApplyService::new(ConfigReconcileService::from_app_context(
        app,
        ConfigReconcileRepository::new(app.db_pool()),
    )?);
    let patches = service.export_patches(&args.planes).await?;
    let output = match &args.out {
        Some(dir) => write_patches(&patches, dir, args.force)?,
        None => print_patches(&patches),
    };
    render_result(&output, &ctx.cli);
    Ok(())
}

fn print_patches(patches: &[PlanePatch]) -> CommandOutput {
    if patches.is_empty() {
        return CommandOutput::text(
            "No database changes to export: every plane matches its last apply",
        );
    }
    let text = patches
        .iter()
        .map(|p| {
            format!(
                "# {} ({})\n{}",
                p.declared_path,
                p.entities.join(", "),
                p.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    CommandOutput::copy_paste_titled("Exported declarations", text)
}

fn write_patches(
    patches: &[PlanePatch],
    dir: &std::path::Path,
    force: bool,
) -> Result<CommandOutput> {
    for patch in patches {
        let target = dir.join(&patch.declared_path);
        if target.exists() && !force {
            bail!(
                "{} already exists; pass --force to replace it",
                target.display()
            );
        }
    }
    let mut lines = Vec::with_capacity(patches.len());
    for patch in patches {
        let target = dir.join(&patch.declared_path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target, &patch.text)?;
        lines.push(format!(
            "{} -> {} ({} entities)",
            patch.plane,
            target.display(),
            patch.entities.len()
        ));
    }
    if lines.is_empty() {
        return Ok(CommandOutput::text(
            "No database changes to export: every plane matches its last apply",
        ));
    }
    Ok(CommandOutput::text(lines.join("\n")))
}
