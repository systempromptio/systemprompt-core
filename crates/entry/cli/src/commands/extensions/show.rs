use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_extension::{ExtensionRegistry, SchemaSource};

use super::types::{
    ExtensionDetailOutput, ExtensionSource, JobInfo, LlmProviderInfo, RoleInfo, SchemaInfo,
    TemplateInfo, ToolInfo,
};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct ShowArgs {
    #[arg(help = "Extension ID to show")]
    pub id: String,
}

pub fn execute(
    args: &ShowArgs,
    _config: &CliConfig,
) -> Result<CommandResult<ExtensionDetailOutput>> {
    let registry = ExtensionRegistry::discover();

    let ext = registry
        .get(&args.id)
        .ok_or_else(|| anyhow!("Extension '{}' not found", args.id))?;

    let jobs: Vec<JobInfo> = ext
        .jobs()
        .iter()
        .map(|job| JobInfo {
            name: job.name().to_string(),
            schedule: job.schedule().to_string(),
            enabled: job.enabled(),
        })
        .collect();

    let templates: Vec<TemplateInfo> = ext
        .template_providers()
        .iter()
        .flat_map(|provider| {
            provider
                .templates()
                .iter()
                .map(|t| TemplateInfo {
                    name: t.name.clone(),
                    description: t.content_types.join(", "),
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let schemas: Vec<SchemaInfo> = ext
        .schemas()
        .iter()
        .map(|schema| {
            let source = match &schema.sql {
                SchemaSource::Inline(_) => "inline".to_string(),
                SchemaSource::File(path) => path.display().to_string(),
            };
            SchemaInfo {
                table: schema.table.clone(),
                source,
                required_columns: schema.required_columns.clone(),
            }
        })
        .collect();

    let tools: Vec<ToolInfo> = ext
        .tool_providers()
        .iter()
        .map(|_provider| ToolInfo {
            name: "tool_provider".to_string(),
        })
        .collect();

    let roles: Vec<RoleInfo> = ext
        .roles()
        .iter()
        .map(|role| RoleInfo {
            name: role.name.clone(),
            display_name: role.display_name.clone(),
            description: role.description.clone(),
            permissions: role.permissions.clone(),
        })
        .collect();

    let llm_providers: Vec<LlmProviderInfo> = ext
        .llm_providers()
        .iter()
        .map(|_provider| LlmProviderInfo {
            name: "llm_provider".to_string(),
        })
        .collect();

    let storage_paths: Vec<String> = ext
        .required_storage_paths()
        .iter()
        .map(|s| (*s).to_string())
        .collect();

    let dependencies: Vec<String> = ext
        .dependencies()
        .iter()
        .map(|s| (*s).to_string())
        .collect();

    let output = ExtensionDetailOutput {
        id: ext.id().to_string(),
        name: ext.name().to_string(),
        version: ext.version().to_string(),
        priority: ext.priority(),
        source: ExtensionSource::Compiled,
        dependencies,
        config_prefix: ext.config_prefix().map(String::from),
        jobs,
        templates,
        schemas,
        routes: vec![],
        tools,
        roles,
        llm_providers,
        storage_paths,
    };

    Ok(CommandResult::card(output).with_title(format!("Extension: {}", args.id)))
}
