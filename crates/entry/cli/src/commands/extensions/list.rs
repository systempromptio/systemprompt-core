use anyhow::Result;
use clap::Args;
use systemprompt_extension::ExtensionRegistry;

use super::types::{CapabilitySummary, ExtensionListOutput, ExtensionSource, ExtensionSummary};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct ListArgs {
    #[arg(long, help = "Filter by extension ID (substring match)")]
    pub filter: Option<String>,

    #[arg(long, value_parser = ["jobs", "templates", "schemas", "routes", "tools", "roles", "llm", "storage"])]
    pub capability: Option<String>,
}

pub fn execute(args: ListArgs, _config: &CliConfig) -> Result<CommandResult<ExtensionListOutput>> {
    let registry = ExtensionRegistry::discover();

    let mut extensions: Vec<ExtensionSummary> = registry
        .extensions()
        .iter()
        .filter(|ext| {
            if let Some(ref filter) = args.filter {
                if !ext.id().to_lowercase().contains(&filter.to_lowercase())
                    && !ext.name().to_lowercase().contains(&filter.to_lowercase())
                {
                    return false;
                }
            }

            if let Some(ref cap) = args.capability {
                match cap.as_str() {
                    "jobs" => return ext.has_jobs(),
                    "templates" => return ext.has_template_providers(),
                    "schemas" => return ext.has_schemas(),
                    "routes" => return false,
                    "tools" => return ext.has_tool_providers(),
                    "roles" => return ext.has_roles(),
                    "llm" => return ext.has_llm_providers(),
                    "storage" => return ext.has_storage_paths(),
                    _ => {},
                }
            }

            true
        })
        .map(|ext| {
            let capabilities = CapabilitySummary {
                jobs: ext.jobs().len(),
                templates: ext.template_providers().len(),
                schemas: ext.schemas().len(),
                routes: 0,
                tools: ext.tool_providers().len(),
                roles: ext.roles().len(),
                llm_providers: ext.llm_providers().len(),
                storage_paths: ext.required_storage_paths().len(),
            };

            ExtensionSummary {
                id: ext.id().to_string(),
                name: ext.name().to_string(),
                version: ext.version().to_string(),
                priority: ext.priority(),
                source: ExtensionSource::Compiled,
                enabled: true,
                capabilities,
            }
        })
        .collect();

    extensions.sort_by_key(|e| e.priority);

    let total = extensions.len();

    let output = ExtensionListOutput { extensions, total };

    Ok(CommandResult::table(output)
        .with_title("Extensions")
        .with_columns(vec![
            "id".to_string(),
            "name".to_string(),
            "version".to_string(),
            "priority".to_string(),
            "source".to_string(),
            "capabilities".to_string(),
        ]))
}
