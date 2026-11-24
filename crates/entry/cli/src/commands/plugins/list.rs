//! `plugins list` command over the admitted composition and the
//! manifest-discovered extensions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use clap::{Args, ValueEnum};
use systemprompt_extension::cli::{CommandError, CommandOutput};
use systemprompt_identifiers::ExtensionId;
use systemprompt_loader::ExtensionLoader;

use super::composition;
use super::types::{
    CapabilitySummary, ExtensionListOutput, ExtensionSource, ExtensionStanding, ExtensionSummary,
};

#[derive(Debug, Clone, Args)]
pub struct ListArgs {
    #[arg(long, help = "Filter by extension ID (substring match)")]
    pub filter: Option<String>,

    #[arg(long, value_enum)]
    pub capability: Option<CapabilityFilter>,

    #[arg(long, value_parser = ["compiled", "manifest", "cli", "mcp", "all"], default_value = "all", help = "Filter by extension type")]
    pub r#type: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CapabilityFilter {
    Jobs,
    Schemas,
    Roles,
    Configs,
}

impl CapabilityFilter {
    const fn held_by(self, capabilities: &CapabilitySummary) -> bool {
        match self {
            Self::Jobs => capabilities.jobs > 0,
            Self::Schemas => capabilities.schemas > 0,
            Self::Roles => capabilities.roles > 0,
            Self::Configs => capabilities.configs > 0,
        }
    }
}

pub fn execute(args: &ListArgs, host: &dyn KernelHost) -> Result<CommandOutput, CommandError> {
    let mut extensions: Vec<ExtensionSummary> = Vec::new();

    if matches!(args.r#type.as_str(), "all" | "compiled") {
        let built = composition::built(host)?;
        extensions.extend(
            composition::matching(built, args.filter.as_deref())
                .map(|member| (member, composition::capabilities(member)))
                .filter(|(_, capabilities)| {
                    args.capability
                        .is_none_or(|filter| filter.held_by(capabilities))
                })
                .map(|(member, capabilities)| ExtensionSummary {
                    id: member.owner.clone(),
                    name: member.metadata.name.to_owned(),
                    version: member.metadata.version.to_owned(),
                    standing: composition::standing(member),
                    source: ExtensionSource::Compiled,
                    enabled: true,
                    capabilities,
                }),
        );
    }

    if matches!(args.r#type.as_str(), "all" | "manifest" | "cli" | "mcp") {
        extensions.extend(collect_manifest(args)?);
    }

    let total = extensions.len();
    let output = ExtensionListOutput { extensions, total };

    Ok(CommandOutput::table_of(
        vec![
            "id",
            "name",
            "version",
            "standing",
            "source",
            "capabilities",
        ],
        &output.extensions,
    )
    .with_title("Extensions"))
}

fn collect_manifest(args: &ListArgs) -> Result<Vec<ExtensionSummary>, CommandError> {
    let project_root = std::env::current_dir().map_err(CommandError::new)?;

    Ok(ExtensionLoader::discover(&project_root)
        .into_iter()
        .filter(|ext| match args.r#type.as_str() {
            "cli" => ext.is_cli(),
            "mcp" => ext.is_mcp(),
            _ => true,
        })
        .filter(|ext| {
            args.filter.as_ref().is_none_or(|filter| {
                ext.manifest
                    .extension
                    .name
                    .to_lowercase()
                    .contains(&filter.to_lowercase())
            })
        })
        .map(|ext| ExtensionSummary {
            id: ExtensionId::new(ext.manifest.extension.name.clone()),
            name: ext.manifest.extension.name.clone(),
            version: "manifest".to_owned(),
            standing: ExtensionStanding::Manifest,
            source: ExtensionSource::Manifest,
            enabled: ext.is_enabled(),
            capabilities: CapabilitySummary::default(),
        })
        .collect())
}
