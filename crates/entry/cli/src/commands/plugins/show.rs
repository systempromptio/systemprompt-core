use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_loader::ExtensionLoader;

use super::types::{
    ExtensionDetailOutput, ExtensionSource, JobInfo, LlmProviderInfo, RoleInfo, SchemaInfo,
    TemplateInfo, ToolInfo,
};
use crate::CliConfig;
use crate::shared::CommandResult;

#[derive(Debug, Clone, Args)]
pub struct ShowArgs {
    #[arg(help = "Extension ID to show")]
    pub id: String,
}

pub(crate) fn execute(
    args: &ShowArgs,
    _config: &CliConfig,
) -> Result<CommandResult<ExtensionDetailOutput>> {
    let registry = ExtensionRegistry::discover()?;
    let needle = args.id.to_lowercase();

    let ext = registry.get(&args.id).or_else(|| {
        registry
            .extensions()
            .iter()
            .find(|e| e.id().to_lowercase() == needle || e.name().to_lowercase() == needle)
    });

    let Some(ext) = ext else {
        return show_manifest(&args.id)
            .ok_or_else(|| anyhow!("Extension '{}' not found", args.id))?;
    };

    let jobs: Vec<JobInfo> = ext
        .jobs()
        .iter()
        .map(|job| JobInfo {
            name: job.name().to_owned(),
            schedule: job.schedule().to_owned(),
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
        .map(|schema| SchemaInfo {
            table: schema.table.clone(),
            source: "inline".to_owned(),
            required_columns: schema.required_columns.clone(),
        })
        .collect();

    let tools: Vec<ToolInfo> = ext
        .tool_providers()
        .iter()
        .map(|_provider| ToolInfo {
            name: "tool_provider".to_owned(),
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
            name: "llm_provider".to_owned(),
        })
        .collect();

    let storage_paths: Vec<String> = ext
        .required_storage_paths()
        .iter()
        .map(|s| (*s).to_owned())
        .collect();

    let dependencies: Vec<String> = ext
        .dependencies()
        .iter()
        .map(|s| (*s).to_owned())
        .collect();

    let output = ExtensionDetailOutput {
        id: systemprompt_identifiers::PluginId::new(ext.id()),
        name: ext.name().to_owned(),
        version: ext.version().to_owned(),
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

fn show_manifest(id: &str) -> Option<Result<CommandResult<ExtensionDetailOutput>>> {
    let project_root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::new());
    let needle = id.to_lowercase();
    let ext = ExtensionLoader::discover(&project_root)
        .into_iter()
        .find(|e| e.manifest.extension.name.to_lowercase() == needle)?;

    let name = ext.manifest.extension.name;
    let output = ExtensionDetailOutput {
        id: systemprompt_identifiers::PluginId::new(name.clone()),
        name: name.clone(),
        version: "manifest".to_owned(),
        priority: 100,
        source: ExtensionSource::Manifest,
        dependencies: vec![],
        config_prefix: None,
        jobs: vec![],
        templates: vec![],
        schemas: vec![],
        routes: vec![],
        tools: vec![],
        roles: vec![],
        llm_providers: vec![],
        storage_paths: vec![],
    };

    Some(Ok(
        CommandResult::card(output).with_title(format!("Extension: {}", name))
    ))
}
