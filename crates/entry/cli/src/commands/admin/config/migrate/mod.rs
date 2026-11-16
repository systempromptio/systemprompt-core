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
//! the retired plugin `hooks.evaluation` key becomes `hooks.judge` ([`hooks`]),
//! and a Slack app's `authz.link_by_workspace_email` becomes
//! `authz.link.link_by_email` ([`slack`]).
//! `--profiles <dir>` instead rewrites the pre-0.73 profiles in that directory
//! ([`profiles`]) without loading one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod abort;
mod commit;
pub mod data;
pub mod fold;
pub mod gateway;
pub mod hooks;
pub mod legacy;
pub mod legacy_map;
mod legacy_wire;
pub mod models;
pub mod plan;
pub mod profile_layer;
pub mod profiles;
pub mod replay;
pub mod roles;
pub mod routes;
pub mod slack;
pub mod swap;
pub mod write;

use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::Args;

use crate::shared::CommandOutput;
use super::types::ConfigMutationOutput;
use systemprompt_extension::cli::CommandHost;

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

    #[arg(
        long,
        value_name = "DIR",
        conflicts_with_all = ["data", "services"],
        help = "Rewrite the pre-0.73 profiles in DIR (e.g. .systemprompt/profiles) instead of the services tree; loads no profile"
    )]
    pub profiles: Option<PathBuf>,
}

fn services_dir(args: &MigrateArgs, host: &dyn CommandHost) -> Result<PathBuf> {
    match args.services.as_ref() {
        Some(dir) => Ok(dir.clone()),
        None => Ok(PathBuf::from(&host.profile()?.paths.services)),
    }
}

#[derive(Debug, Default)]
struct MigrationSteps(Vec<ConfigMutationOutput>);

impl MigrationSteps {
    fn push(&mut self, field: &str, message: String) {
        self.0.push(ConfigMutationOutput {
            field: field.to_owned(),
            message,
        });
    }

    fn output(self, title: &str) -> CommandOutput {
        CommandOutput::table_of(vec!["field", "message"], &self.0).with_title(title)
    }
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

fn execute_profiles(args: &MigrateArgs, dir: &std::path::Path) -> Result<CommandOutput> {
    let mut steps = MigrationSteps::default();
    if args.check {
        let pending = profiles::plan(dir)?;
        if pending.is_empty() {
            steps.push("profiles", profiles::describe(&pending));
            return Ok(steps.output("Profile Migration"));
        }
        bail!(
            "a profile migration is pending; run `systemprompt admin config migrate --profiles {}`\n{}",
            dir.display(),
            profiles::describe(&pending)
        );
    }
    let written = profiles::apply(dir)?;
    steps.push("profiles", profiles::describe(&written));
    Ok(steps.output("Profiles Migrated"))
}

pub async fn execute(args: &MigrateArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    match args.profiles.as_ref() {
        Some(dir) => execute_profiles(args, dir),
        None => execute_services(args, host).await,
    }
}

fn refuse_pending_renames(services: &std::path::Path) -> Result<()> {
    if hooks::pending(services) {
        bail!(
            "plugin descriptors still say `hooks.evaluation`; run `systemprompt admin config migrate` to rename it to `hooks.judge`"
        );
    }
    if slack::pending(services) {
        bail!(
            "a Slack app still says `authz.link_by_workspace_email`; run `systemprompt admin config migrate` to rename it to `authz.link.link_by_email`"
        );
    }
    Ok(())
}

fn apply_renames(steps: &mut MigrationSteps, services: &std::path::Path) -> Result<()> {
    for path in hooks::migrate(services)? {
        steps.push(
            "plugin_hooks",
            format!(
                "renamed hooks.evaluation to hooks.judge in {}",
                path.display()
            ),
        );
    }
    for path in slack::migrate(services)? {
        steps.push(
            "slack_link_policy",
            format!(
                "renamed authz.link_by_workspace_email to authz.link.link_by_email in {}",
                path.display()
            ),
        );
    }
    Ok(())
}

async fn execute_services(args: &MigrateArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let mut steps = MigrationSteps::default();
    let services = services_dir(args, host)?;
    let pending = plan(&services)?;
    if args.check {
        if roles::pending(&services) {
            bail!(
                "{} is retired; run `systemprompt admin config migrate` to fold it into {}",
                roles::ROLES_FILE,
                systemprompt_governance::authz::rules_file::RULES_FILE
            );
        }
        refuse_pending_renames(&services)?;
        let Some(plan) = pending else {
            steps.push(
                "catalog",
                "the services tree is already in the 0.73 shape".to_owned(),
            );
            return Ok(steps.output("Catalog Migration"));
        };
        write::verify(&services, &plan)?;
        bail!(
            "a catalog migration is pending; run `systemprompt admin config migrate`\n{}",
            describe(&plan, &[])
        );
    }
    match pending {
        Some(plan) => {
            let written = write::apply(
                &services,
                &plan,
                &crate::commands::admin::state::get::<crate::runner::interrupt::InterruptShield>(host).await?,
            )?;
            steps.push("catalog", describe(&plan, &written));
        },
        None => steps.push(
            "catalog",
            "nothing to migrate: the services tree is already in the 0.73 shape".to_owned(),
        ),
    }
    apply_renames(&mut steps, &services)?;
    if let Some(rules) = roles::migrate(&services)? {
        steps.push(
            "access_control",
            format!("folded {} into {}", roles::ROLES_FILE, rules.display()),
        );
    }
    if args.data {
        let map = data::read_map(&services)?;
        let report = data::apply_with(&systemprompt_database::pool_from_handle(host.db().await?)?, &map).await?;
        steps.push(
            "catalog_data",
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
    Ok(steps.output("Config Migration"))
}
