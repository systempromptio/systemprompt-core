use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::PluginId;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
pub struct CapabilitySummary {
    pub jobs: usize,
    pub templates: usize,
    pub schemas: usize,
    pub routes: usize,
    pub tools: usize,
    pub roles: usize,
    pub llm_providers: usize,
    pub storage_paths: usize,
}

impl CapabilitySummary {
    pub fn summary_string(&self) -> String {
        let mut parts = Vec::new();

        if self.jobs > 0 {
            parts.push(format!(
                "{} {}",
                self.jobs,
                if self.jobs == 1 { "job" } else { "jobs" }
            ));
        }
        if self.templates > 0 {
            parts.push(format!(
                "{} {}",
                self.templates,
                if self.templates == 1 {
                    "template"
                } else {
                    "templates"
                }
            ));
        }
        if self.schemas > 0 {
            parts.push(format!(
                "{} {}",
                self.schemas,
                if self.schemas == 1 {
                    "schema"
                } else {
                    "schemas"
                }
            ));
        }
        if self.routes > 0 {
            parts.push(format!(
                "{} {}",
                self.routes,
                if self.routes == 1 { "route" } else { "routes" }
            ));
        }
        if self.tools > 0 {
            parts.push(format!(
                "{} {}",
                self.tools,
                if self.tools == 1 { "tool" } else { "tools" }
            ));
        }
        if self.roles > 0 {
            parts.push(format!(
                "{} {}",
                self.roles,
                if self.roles == 1 { "role" } else { "roles" }
            ));
        }
        if self.llm_providers > 0 {
            parts.push(format!(
                "{} {}",
                self.llm_providers,
                if self.llm_providers == 1 {
                    "LLM"
                } else {
                    "LLMs"
                }
            ));
        }

        if parts.is_empty() {
            "none".to_string()
        } else {
            parts.join(", ")
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobWithExtension {
    pub extension_id: PluginId,
    pub extension_name: String,
    pub job_name: String,
    pub schedule: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobsListOutput {
    pub jobs: Vec<JobWithExtension>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TemplateWithExtension {
    pub extension_id: PluginId,
    pub extension_name: String,
    pub template_name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TemplatesListOutput {
    pub templates: Vec<TemplateWithExtension>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SchemaWithExtension {
    pub extension_id: PluginId,
    pub extension_name: String,
    pub table: String,
    pub source: String,
    pub migration_weight: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SchemasListOutput {
    pub schemas: Vec<SchemaWithExtension>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RouteWithExtension {
    pub extension_id: PluginId,
    pub extension_name: String,
    pub base_path: String,
    pub requires_auth: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RoutesListOutput {
    pub routes: Vec<RouteWithExtension>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolWithExtension {
    pub extension_id: PluginId,
    pub extension_name: String,
    pub tool_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolsListOutput {
    pub tools: Vec<ToolWithExtension>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RoleWithExtension {
    pub extension_id: PluginId,
    pub extension_name: String,
    pub role_name: String,
    pub display_name: String,
    pub description: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RolesListOutput {
    pub roles: Vec<RoleWithExtension>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LlmProviderWithExtension {
    pub extension_id: PluginId,
    pub extension_name: String,
    pub provider_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LlmProvidersListOutput {
    pub providers: Vec<LlmProviderWithExtension>,
    pub total: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct CapabilitiesSummaryOutput {
    pub jobs: usize,
    pub templates: usize,
    pub schemas: usize,
    pub tools: usize,
    pub roles: usize,
    pub llm_providers: usize,
    pub extension_count: usize,
}
