//! `plugins seams`: the admitted composition, member by member.
//!
//! Every member the composition admitted against the active profile is listed
//! with what it contributes to the kernel, one row per contribution kind it
//! uses: provided and required state values, capability slots it opens or
//! contributes to, schemas, migrations, jobs, retention and purge
//! declarations, projections, run origins, governance providers and
//! policies, gateway guards, readiness checks, diagnostics, metrics, roles
//! and configuration sections. A member the profile disables is not admitted
//! and has no rows; a profile setting that needs a contribution no admitted
//! member makes is refused when the composition is admitted, before any
//! command runs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use clap::Args;
use crate::kernel::KernelHost;
use systemprompt_extension::admitted::AdmittedMember;
use systemprompt_extension::cli::{CommandError, CommandOutput};

use super::composition;
use super::types::SeamRow;

#[derive(Debug, Clone, Args)]
pub struct SeamsArgs {
    #[arg(long, help = "Show only members whose id or name contains this text")]
    pub member: Option<String>,
}

pub fn execute(args: &SeamsArgs, host: &dyn KernelHost) -> Result<CommandOutput, CommandError> {
    let built = composition::built(host)?;
    let rows: Vec<SeamRow> = composition::matching(built, args.member.as_deref())
        .flat_map(member_rows)
        .collect();
    Ok(CommandOutput::table_of(
        vec!["member", "standing", "contribution", "count", "entries"],
        &rows,
    )
    .with_title("Admitted Composition"))
}

#[must_use]
pub fn member_rows(member: &AdmittedMember) -> Vec<SeamRow> {
    let sink = &member.sink;
    let named = |contribution: &str, entries: Vec<String>| (contribution.to_owned(), entries.len(), entries.join(", "));
    let counted = |contribution: &str, count: usize| (contribution.to_owned(), count, String::new());
    [
        counted("provides", sink.provides().len()),
        counted("requires", sink.requires().len()),
        counted("slots", sink.slots().len()),
        named(
            "schemas",
            sink.schemas().iter().filter_map(|schema| schema.table.clone()).collect(),
        ),
        counted("migrations", sink.migrations().len()),
        counted("seeds", sink.seeds().len()),
        named(
            "jobs",
            sink.jobs().iter().map(|job| job.job().name().to_owned()).collect(),
        ),
        counted("job_run_scopes", sink.job_run_scopes().len()),
        counted("retention", sink.retention().len()),
        counted("user_purges", sink.user_purges().len()),
        counted("orphan_sweeps", sink.orphan_sweeps().len()),
        counted("projections", sink.projections().len()),
        counted("run_origins", sink.run_origins().len()),
        counted("subject_providers", sink.subject_providers().len()),
        counted("policies", sink.policies().len()),
        counted("gateway_guards", sink.gateway_guards().len()),
        counted("runner_services", sink.runner_services().len()),
        counted("readiness", sink.readiness().len()),
        counted("diagnostics", sink.diagnostics().len()),
        counted("metrics", sink.metrics().len()),
        named("roles", sink.roles().iter().map(|role| role.name.clone()).collect()),
        named(
            "configs",
            sink.configs().iter().map(|config| config.prefix().to_owned()).collect(),
        ),
        counted("config_planes", sink.config_planes().len()),
        counted("assets", sink.assets().len()),
    ]
    .into_iter()
    .filter(|(_, count, _)| *count > 0)
    .map(|(contribution, count, entries)| SeamRow {
        member: member.owner.clone(),
        standing: composition::standing(member),
        contribution,
        count,
        entries,
    })
    .collect()
}
