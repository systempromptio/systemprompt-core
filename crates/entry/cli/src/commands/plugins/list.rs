use clap::Args;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_loader::ExtensionLoader;

use super::types::{CapabilitySummary, ExtensionListOutput, ExtensionSource, ExtensionSummary};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct ListArgs {
    #[arg(long, help = "Filter by extension ID (substring match)")]
    pub filter: Option<String>,

    #[arg(long, value_parser = ["jobs", "templates", "schemas", "routes", "tools", "roles", "llm", "storage"])]
    pub capability: Option<String>,

    #[arg(long, value_parser = ["compiled", "manifest", "cli", "mcp", "all"], default_value = "all", help = "Filter by extension type")]
    pub r#type: String,
}

pub fn execute(args: &ListArgs, _config: &CliConfig) -> CommandResult<ExtensionListOutput> {
    let mut extensions: Vec<ExtensionSummary> = Vec::new();

    let include_compiled = matches!(args.r#type.as_str(), "all" | "compiled");
    let include_manifest = matches!(args.r#type.as_str(), "all" | "manifest" | "cli" | "mcp");

    if include_compiled {
        let registry = ExtensionRegistry::discover();

        extensions.extend(
            registry
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
                }),
        );
    }

    if include_manifest {
        let project_root = std::env::current_dir().unwrap_or_default();
        let discovered = ExtensionLoader::discover(&project_root);

        for ext in discovered {
            let type_matches = match args.r#type.as_str() {
                "cli" => ext.is_cli(),
                "mcp" => ext.is_mcp(),
                _ => true,
            };

            if !type_matches {
                continue;
            }

            if let Some(ref filter) = args.filter {
                if !ext
                    .manifest
                    .extension
                    .name
                    .to_lowercase()
                    .contains(&filter.to_lowercase())
                {
                    continue;
                }
            }

            extensions.push(ExtensionSummary {
                id: ext.manifest.extension.name.clone(),
                name: ext.manifest.extension.name.clone(),
                version: "manifest".to_string(),
                priority: 100,
                source: ExtensionSource::Manifest,
                enabled: ext.is_enabled(),
                capabilities: CapabilitySummary::default(),
            });
        }
    }

    extensions.sort_by_key(|e| e.priority);

    let total = extensions.len();

    let output = ExtensionListOutput { extensions, total };

    CommandResult::table(output)
        .with_title("Extensions")
        .with_columns(vec![
            "id".to_string(),
            "name".to_string(),
            "version".to_string(),
            "priority".to_string(),
            "source".to_string(),
            "capabilities".to_string(),
        ])
}
