//! `plugins seams`: every link-time registration seam and what this binary
//! registers in it.
//!
//! Registrations collected with `inventory` exist only in a binary that links
//! the registering crate. The command lists each seam with its entries
//! (including seams with none) and checks the active profile for settings
//! that name a seam entry this binary does not link: such a profile fails at
//! boot, so the command exits non-zero.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

macro_rules! by_priority {
    ($discover:expr) => {
        prioritized(
            $discover
                .into_iter()
                .map(|registration| registration.priority),
        )
    };
}

macro_rules! by_count {
    ($registration:ty) => {
        unnamed(inventory::iter::<$registration>.into_iter().count())
    };
}

mod app_seams;
pub mod catalog;
mod extension_seams;

use clap::Args;
use systemprompt_manifest::Profile;
use systemprompt_manifest::profile::{AuthzMode, SecretsSource};

pub use catalog::{LinkedSeam, linked_seams};

use super::types::{SeamRow, SeamStatus};
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Args)]
pub struct SeamsArgs {
    #[arg(long, help = "Show only seams whose name contains this text")]
    pub seam: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Needed {
    AnyEntry,
    Entry(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requirement {
    pub seam: &'static str,
    pub setting: String,
    pub needed: Needed,
}

#[must_use]
pub fn profile_requirements(profile: &Profile) -> Vec<Requirement> {
    let mut requirements = Vec::new();
    let authz_mode = profile
        .governance
        .as_ref()
        .and_then(|governance| governance.authz.as_ref())
        .map(|authz| authz.hook.mode);
    if authz_mode == Some(AuthzMode::Extension) {
        requirements.push(Requirement {
            seam: "authz_hooks",
            setting: "governance.authz.hook.mode = extension".to_owned(),
            needed: Needed::AnyEntry,
        });
    }
    if let Some(secrets) = profile.secrets.as_ref()
        && secrets.source == SecretsSource::External
        && let Some(external) = secrets.external.as_ref()
    {
        requirements.push(Requirement {
            seam: "secrets_sources",
            setting: format!("secrets.external.kind = {}", external.kind),
            needed: Needed::Entry(external.kind.clone()),
        });
    }
    requirements
}

fn satisfied(seam: &LinkedSeam, needed: &Needed) -> bool {
    match needed {
        Needed::AnyEntry => !seam.entries.is_empty(),
        Needed::Entry(name) => seam.entries.iter().any(|entry| entry == name),
    }
}

#[must_use]
pub fn evaluate(seams: Vec<LinkedSeam>, requirements: &[Requirement]) -> (Vec<SeamRow>, bool) {
    let mut all_linked = true;
    let rows = seams
        .into_iter()
        .map(|seam| {
            let applicable: Vec<&Requirement> = requirements
                .iter()
                .filter(|requirement| requirement.seam == seam.name)
                .collect();
            let missing = applicable
                .iter()
                .any(|requirement| !satisfied(&seam, &requirement.needed));
            all_linked &= !missing;
            let status = if missing {
                SeamStatus::Missing
            } else if seam.entries.is_empty() {
                SeamStatus::Empty
            } else {
                SeamStatus::Linked
            };
            SeamRow {
                seam: seam.name.to_owned(),
                owner: seam.owner.to_owned(),
                count: seam.entries.len(),
                entries: seam.entries.join(", "),
                requires: applicable
                    .iter()
                    .map(|requirement| requirement.setting.as_str())
                    .collect::<Vec<_>>()
                    .join("; "),
                status,
            }
        })
        .collect();
    (rows, all_linked)
}

#[must_use]
pub fn execute(args: &SeamsArgs, profile: Option<&Profile>) -> (CommandOutput, bool) {
    let requirements = profile.map(profile_requirements).unwrap_or_default();
    let (rows, all_linked) = evaluate(linked_seams(), &requirements);
    let rows: Vec<SeamRow> = rows
        .into_iter()
        .filter(|row| {
            args.seam
                .as_ref()
                .is_none_or(|f| row.seam.contains(f.as_str()))
        })
        .collect();
    let output = CommandOutput::table_of(
        vec!["seam", "owner", "count", "entries", "requires", "status"],
        &rows,
    )
    .with_title("Registration Seams Linked In This Binary");
    (output, all_linked)
}
