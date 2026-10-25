//! `systemprompt admin config migrate [--check] [--data]`: the one-shot
//! rewrite of a pre-0.73 `services/` tree to the catalog shape.
//!
//! The config half reads the legacy catalog (the only code that parses it,
//! [`legacy`]), replays 0.72.1 dispatch over it ([`replay`]) and writes
//! `services/ai/{credentials,providers,models,deployments,gateway}.yaml`, the
//! rewritten `ai/config.yaml` and root `includes:`, and `ai/legacy_map.yaml`
//! — only after the result has loaded through the real loader
//! ([`write`](mod@write)). Anything it cannot decide aborts with the file and
//! keys ([`abort`]); a second run finds no legacy shape and writes nothing.
//! `--check` reports what would change and fails while a migration is pending.
//! `--data` then moves stored data through the legacy map ([`data`]),
//! including access rules addressed to a legacy route id. The retired
//! `access-control/roles.yaml` is folded into `rules.yaml` ([`roles`]), and
//! the retired plugin `hooks.evaluation` key becomes `hooks.judge` ([`hooks`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod abort;
pub mod data;
pub mod fold;
pub mod gateway;
pub mod hooks;
pub mod legacy;
pub mod legacy_map;
pub mod models;
pub mod plan;
pub mod replay;
pub mod roles;
pub mod routes;
pub mod write;

use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::Args;
use systemprompt_config::ProfileBootstrap;

use super::types::ConfigMutationOutput;
use crate::CliConfig;
use crate::context::CommandContext;
use crate::shared::{CommandOutput, render_result};

pub use plan::{MigrationPlan, legacy_files, plan};

#[derive(Debug, Clone, Args)]
pub struct MigrateArgs {
    #[arg(
        long,
        help = "Report what would change; fail while a migration is pending"
    )]
    pub check: bool,

    #[arg(
        long,
        help = "Also move stored data through ai/legacy_map.yaml (needs the database)"
    )]
    pub data: bool,

    #[arg(
        long,
        help = "services/ directory to migrate (defaults to the active profile's; required with --database-url)"
    )]
    pub services: Option<PathBuf>,
}

fn services_dir(args: &MigrateArgs) -> Result<PathBuf> {
    match args.services.as_ref() {
        Some(dir) => Ok(dir.clone()),
        None => Ok(PathBuf::from(&ProfileBootstrap::get()?.paths.services)),
    }
}

fn render(config: &CliConfig, title: &str, message: String) {
    render_result(
        &CommandOutput::card_value(
            title,
            &ConfigMutationOutput {
                field: "catalog".to_owned(),
                message,
            },
        ),
        config,
    );
}

fn describe(plan: &MigrationPlan, written: &[PathBuf]) -> String {
    let mut lines: Vec<String> = written
        .iter()
        .map(|p| format!("wrote {}", p.display()))
        .collect();
    lines.extend(plan.notes.iter().map(|n| format!("note: {n}")));
    lines.push(format!(
        "{} legacy entries mapped, {} legacy routes mapped, {} undispatchable models dropped",
        plan.legacy_map.providers.len(),
        plan.legacy_map.route_models.len(),
        plan.legacy_map.dropped_models.len()
    ));
    lines.join("\n")
}

pub async fn execute(args: &MigrateArgs, ctx: &CommandContext) -> Result<()> {
    let config = &ctx.cli;
    let services = services_dir(args)?;
    let pending = plan(&services)?;
    if args.check {
        if roles::pending(&services) {
            bail!(
                "{} is retired; run `systemprompt admin config migrate` to fold it into {}",
                roles::ROLES_FILE,
                systemprompt_governance::authz::rules_file::RULES_FILE
            );
        }
        if hooks::pending(&services) {
            bail!(
                "plugin descriptors still say `hooks.evaluation`; run `systemprompt admin config migrate` to rename it to `hooks.judge`"
            );
        }
        let Some(plan) = pending else {
            render(
                config,
                "Catalog Migration",
                "the services tree is already in the 0.73 shape".to_owned(),
            );
            return Ok(());
        };
        write::verify(&services, &plan)?;
        render(config, "Catalog Migration Pending", describe(&plan, &[]));
        bail!("a catalog migration is pending; run `systemprompt admin config migrate`");
    }
    match pending {
        Some(plan) => {
            let written = write::apply(&services, &plan)?;
            render(config, "Catalog Migrated", describe(&plan, &written));
        },
        None => render(
            config,
            "Catalog Migration",
            "nothing to migrate: the services tree is already in the 0.73 shape".to_owned(),
        ),
    }
    for path in hooks::migrate(&services)? {
        render(
            config,
            "Plugin Hooks Migrated",
            format!(
                "renamed hooks.evaluation to hooks.judge in {}",
                path.display()
            ),
        );
    }
    if let Some(rules) = roles::migrate(&services)? {
        render(
            config,
            "Access Control Migrated",
            format!("folded {} into {}", roles::ROLES_FILE, rules.display()),
        );
    }
    if args.data {
        let map = data::read_map(&services)?;
        let report = data::apply_with(&ctx.db_pool().await?, &map).await?;
        render(
            config,
            "Catalog Data Migrated",
            format!(
                "{} legacy entries recorded, {} personal keys moved, {} audit rows backfilled, {} host filters rewritten, {} legacy route rules moved onto model entities",
                report.legacy_entries,
                report.personal_keys_moved,
                report.audit_rows_backfilled,
                report.host_filters_rewritten,
                report.route_rules_moved_to_models
            ),
        );
    }
    Ok(())
}
