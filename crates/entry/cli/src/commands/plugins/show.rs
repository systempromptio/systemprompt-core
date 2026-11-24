//! `plugins show` command: one admitted member's contributions, or a
//! manifest-discovered extension.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_extension::admitted::AdmittedMember;
use systemprompt_extension::cli::{CommandError, CommandOutput};
use systemprompt_identifiers::ExtensionId;
use systemprompt_loader::ExtensionLoader;

use super::composition;
use super::types::{
    ExtensionDetailOutput, ExtensionSource, ExtensionStanding, JobInfo, RoleInfo, SchemaInfo,
};

#[derive(Debug, Clone, Args)]
pub struct ShowArgs {
    #[arg(help = "Extension ID to show")]
    pub id: String,
}

pub fn execute(args: &ShowArgs, host: &dyn KernelHost) -> Result<CommandOutput> {
    let built = composition::built(host)?;
    let output = match composition::find(built, &args.id) {
        Some(member) => build_detail_output(member),
        None => {
            show_manifest(&args.id)?.ok_or_else(|| anyhow!("Extension '{}' not found", args.id))?
        },
    };

    Ok(CommandOutput::card_value(
        format!("Extension: {}", args.id),
        &output,
    ))
}

pub(crate) fn build_detail_output(member: &AdmittedMember) -> ExtensionDetailOutput {
    ExtensionDetailOutput {
        id: member.owner.clone(),
        name: member.metadata.name.to_owned(),
        version: member.metadata.version.to_owned(),
        standing: composition::standing(member),
        source: ExtensionSource::Compiled,
        config_prefixes: member
            .sink
            .configs()
            .iter()
            .map(|config| config.prefix().to_owned())
            .collect(),
        jobs: member
            .sink
            .jobs()
            .iter()
            .map(|contributed| {
                let job = contributed.job();
                JobInfo {
                    name: job.name().to_owned(),
                    schedule: job.schedule().to_owned(),
                    enabled: job.enabled(),
                }
            })
            .collect(),
        schemas: member
            .sink
            .schemas()
            .iter()
            .map(|schema| SchemaInfo {
                table: schema.table.clone(),
                required_columns: schema.required_columns.clone(),
            })
            .collect(),
        roles: member
            .sink
            .roles()
            .iter()
            .map(|role| RoleInfo {
                name: role.name.clone(),
                display_name: role.display_name.clone(),
                description: role.description.clone(),
                permissions: role.permissions.clone(),
            })
            .collect(),
    }
}

fn show_manifest(id: &str) -> Result<Option<ExtensionDetailOutput>, CommandError> {
    let project_root = std::env::current_dir().map_err(CommandError::new)?;
    let needle = id.to_lowercase();
    let Some(ext) = ExtensionLoader::discover(&project_root)
        .into_iter()
        .find(|e| e.manifest.extension.name.to_lowercase() == needle)
    else {
        return Ok(None);
    };

    let name = ext.manifest.extension.name;
    Ok(Some(ExtensionDetailOutput {
        id: ExtensionId::new(name.clone()),
        name,
        version: "manifest".to_owned(),
        standing: ExtensionStanding::Manifest,
        source: ExtensionSource::Manifest,
        config_prefixes: vec![],
        jobs: vec![],
        schemas: vec![],
        roles: vec![],
    }))
}
