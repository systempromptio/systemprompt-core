use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionSource {
    Compiled,
    Manifest,
}

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
pub struct ExtensionSummary {
    pub id: String,
    pub name: String,
    pub version: String,
    pub priority: u32,
    pub source: ExtensionSource,
    pub enabled: bool,
    pub capabilities: CapabilitySummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtensionListOutput {
    pub extensions: Vec<ExtensionSummary>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobInfo {
    pub name: String,
    pub schedule: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TemplateInfo {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SchemaInfo {
    pub table: String,
    pub source: String,
    pub required_columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RouteInfo {
    pub base_path: String,
    pub requires_auth: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolInfo {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RoleInfo {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LlmProviderInfo {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtensionDetailOutput {
    pub id: String,
    pub name: String,
    pub version: String,
    pub priority: u32,
    pub source: ExtensionSource,
    pub dependencies: Vec<String>,
    pub config_prefix: Option<String>,
    pub jobs: Vec<JobInfo>,
    pub templates: Vec<TemplateInfo>,
    pub schemas: Vec<SchemaInfo>,
    pub routes: Vec<RouteInfo>,
    pub tools: Vec<ToolInfo>,
    pub roles: Vec<RoleInfo>,
    pub llm_providers: Vec<LlmProviderInfo>,
    pub storage_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtensionValidationOutput {
    pub valid: bool,
    pub extension_count: usize,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidationError {
    pub extension_id: Option<String>,
    pub error_type: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidationWarning {
    pub extension_id: Option<String>,
    pub warning_type: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtensionConfigOutput {
    pub extension_id: String,
    pub config_prefix: Option<String>,
    pub config_schema: Option<serde_json::Value>,
    pub has_config: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobWithExtension {
    pub extension_id: String,
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
    pub extension_id: String,
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
    pub extension_id: String,
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
    pub extension_id: String,
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
    pub extension_id: String,
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
    pub extension_id: String,
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
    pub extension_id: String,
    pub extension_name: String,
    pub provider_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LlmProvidersListOutput {
    pub providers: Vec<LlmProviderWithExtension>,
    pub total: usize,
}
