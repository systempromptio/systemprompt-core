use clap::Args;
use systemprompt_extension::Extension;
use systemprompt_loader::ExtensionLoader;

use super::discover_registry;
use super::types::{CapabilitySummary, ExtensionListOutput, ExtensionSource, ExtensionSummary};
use crate::CliConfig;
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Args)]
pub struct ListArgs {
    #[arg(long, help = "Filter by extension ID (substring match)")]
    pub filter: Option<String>,

    #[arg(long, value_parser = ["jobs", "templates", "schemas", "routes", "tools", "roles", "llm", "storage"])]
    pub capability: Option<String>,

    #[arg(long, value_parser = ["compiled", "manifest", "cli", "mcp", "all"], default_value = "all", help = "Filter by extension type")]
    pub r#type: String,
}

pub fn execute(args: &ListArgs, _config: &CliConfig) -> CommandOutput {
    let mut extensions: Vec<ExtensionSummary> = Vec::new();

    if matches!(args.r#type.as_str(), "all" | "compiled") {
        extensions.extend(collect_compiled(args));
    }

    if matches!(args.r#type.as_str(), "all" | "manifest" | "cli" | "mcp") {
        extensions.extend(collect_manifest(args));
    }

    extensions.sort_by_key(|e| e.priority);

    let total = extensions.len();

    let output = ExtensionListOutput { extensions, total };

    CommandOutput::table_of(
        vec![
            "id",
            "name",
            "version",
            "priority",
            "source",
            "capabilities",
        ],
        &output.extensions,
    )
    .with_title("Extensions")
}

fn collect_compiled(args: &ListArgs) -> Vec<ExtensionSummary> {
    let registry = discover_registry();
    registry
        .extensions()
        .iter()
        .filter(|ext| matches_compiled_filters(ext.as_ref(), args))
        .map(|ext| compiled_summary(ext.as_ref()))
        .collect()
}

fn matches_compiled_filters(ext: &dyn Extension, args: &ListArgs) -> bool {
    if let Some(ref filter) = args.filter
        && !ext.id().to_lowercase().contains(&filter.to_lowercase())
        && !ext.name().to_lowercase().contains(&filter.to_lowercase())
    {
        return false;
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
}

fn compiled_summary(ext: &dyn Extension) -> ExtensionSummary {
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
        id: systemprompt_identifiers::PluginId::new(ext.id()),
        name: ext.name().to_owned(),
        version: ext.version().to_owned(),
        priority: ext.priority(),
        source: ExtensionSource::Compiled,
        enabled: true,
        capabilities,
    }
}

fn collect_manifest(args: &ListArgs) -> Vec<ExtensionSummary> {
    let project_root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::new());
    let mut summaries = Vec::new();

    for ext in ExtensionLoader::discover(&project_root) {
        let type_matches = match args.r#type.as_str() {
            "cli" => ext.is_cli(),
            "mcp" => ext.is_mcp(),
            _ => true,
        };

        if !type_matches {
            continue;
        }

        if let Some(ref filter) = args.filter
            && !ext
                .manifest
                .extension
                .name
                .to_lowercase()
                .contains(&filter.to_lowercase())
        {
            continue;
        }

        summaries.push(ExtensionSummary {
            id: systemprompt_identifiers::PluginId::new(ext.manifest.extension.name.clone()),
            name: ext.manifest.extension.name.clone(),
            version: "manifest".to_owned(),
            priority: 100,
            source: ExtensionSource::Manifest,
            enabled: ext.is_enabled(),
            capabilities: CapabilitySummary::default(),
        });
    }

    summaries
}
