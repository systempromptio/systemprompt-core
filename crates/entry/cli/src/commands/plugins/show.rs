//! `plugins show` command rendering one extension's jobs, templates, and
//! schemas.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_extension::{Extension, ExtensionRegistry};
use systemprompt_loader::ExtensionLoader;

use super::types::{
    ExtensionDetailOutput, ExtensionSource, JobInfo, LlmProviderInfo, RoleInfo, SchemaInfo,
    TemplateInfo, ToolInfo,
};
use crate::CliConfig;
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Args)]
pub struct ShowArgs {
    #[arg(help = "Extension ID to show")]
    pub id: String,
}

pub fn execute(args: &ShowArgs, _config: &CliConfig) -> Result<CommandOutput> {
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

    let output = build_detail_output(ext.as_ref());

    Ok(CommandOutput::card_value(
        format!("Extension: {}", args.id),
        &output,
    ))
}

pub fn build_detail_output(ext: &dyn Extension) -> ExtensionDetailOutput {
    ExtensionDetailOutput {
        id: systemprompt_identifiers::PluginId::new(ext.id()),
        name: ext.name().to_owned(),
        version: ext.version().to_owned(),
        priority: ext.priority(),
        source: ExtensionSource::Compiled,
        dependencies: ext.dependencies().iter().map(|s| (*s).to_owned()).collect(),
        config_prefix: ext.config_prefix().map(String::from),
        jobs: job_infos(ext),
        templates: template_infos(ext),
        schemas: schema_infos(ext),
        routes: vec![],
        tools: ext
            .tool_providers()
            .iter()
            .map(|_provider| ToolInfo {
                name: "tool_provider".to_owned(),
            })
            .collect(),
        roles: role_infos(ext),
        llm_providers: ext
            .llm_providers()
            .iter()
            .map(|_provider| LlmProviderInfo {
                name: "llm_provider".to_owned(),
            })
            .collect(),
        storage_paths: ext
            .required_storage_paths()
            .iter()
            .map(|s| (*s).to_owned())
            .collect(),
    }
}

fn job_infos(ext: &dyn Extension) -> Vec<JobInfo> {
    ext.jobs()
        .iter()
        .map(|job| JobInfo {
            name: job.name().to_owned(),
            schedule: job.schedule().to_owned(),
            enabled: job.enabled(),
        })
        .collect()
}

fn template_infos(ext: &dyn Extension) -> Vec<TemplateInfo> {
    ext.template_providers()
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
        .collect()
}

fn schema_infos(ext: &dyn Extension) -> Vec<SchemaInfo> {
    ext.schemas()
        .iter()
        .map(|schema| SchemaInfo {
            table: schema.table.clone(),
            source: "inline".to_owned(),
            required_columns: schema.required_columns.clone(),
        })
        .collect()
}

fn role_infos(ext: &dyn Extension) -> Vec<RoleInfo> {
    ext.roles()
        .iter()
        .map(|role| RoleInfo {
            name: role.name.clone(),
            display_name: role.display_name.clone(),
            description: role.description.clone(),
            permissions: role.permissions.clone(),
        })
        .collect()
}

fn show_manifest(id: &str) -> Option<Result<CommandOutput>> {
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

    Some(Ok(CommandOutput::card_value(
        format!("Extension: {}", name),
        &output,
    )))
}
