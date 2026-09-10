//! `marketplace import` — convert an Anthropic-format authoring tree into a
//! systemprompt services tree.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;
use systemprompt_marketplace::{ImportOptions, ImportReport, import_anthropic_tree};

use crate::shared::CommandOutput;

#[derive(Debug, Clone, Args)]
pub struct ImportArgs {
    #[arg(long, help = "Anthropic-format repository to read")]
    pub from: PathBuf,

    #[arg(long, help = "Services tree to write (must not exist or be empty)")]
    pub into: PathBuf,

    #[arg(long, help = "Report what would be written without writing it")]
    pub dry_run: bool,

    #[arg(long, help = "Refuse the import when any warning is raised")]
    pub strict: bool,
}

#[derive(Debug, Serialize)]
struct ImportRow {
    kind: String,
    count: usize,
    entries: String,
}

pub fn execute(args: &ImportArgs) -> Result<CommandOutput> {
    let opts = ImportOptions {
        strict: args.strict,
        dry_run: args.dry_run,
    };
    let report = import_anthropic_tree(&args.from, &args.into, &opts).with_context(|| {
        format!(
            "Failed to import {} into {}",
            args.from.display(),
            args.into.display()
        )
    })?;

    let title = if args.dry_run {
        "Marketplace Import (dry run)"
    } else {
        "Marketplace Import"
    };
    Ok(
        CommandOutput::table_of(vec!["kind", "count", "entries"], &report_rows(&report))
            .with_title(title),
    )
}

fn report_rows(report: &ImportReport) -> Vec<ImportRow> {
    let marketplaces: Vec<String> = report
        .marketplaces
        .iter()
        .map(ToString::to_string)
        .collect();
    let plugins: Vec<String> = report.plugins.iter().map(ToString::to_string).collect();
    let warnings: Vec<String> = report.warnings.iter().map(ToString::to_string).collect();

    vec![
        row("marketplaces", &marketplaces),
        row("plugins", &plugins),
        row("skills", &report.skills),
        row("rules", &report.rules),
        row("hooks", &report.hooks),
        row("base_dirs", &report.copied_base_dirs),
        row("warnings", &warnings),
    ]
}

fn row(kind: &str, entries: &[String]) -> ImportRow {
    ImportRow {
        kind: kind.to_owned(),
        count: entries.len(),
        entries: entries.join(", "),
    }
}
