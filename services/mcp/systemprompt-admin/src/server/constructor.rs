use std::collections::HashMap;
use std::sync::Arc;

use systemprompt::agent::services::mcp::ToolResultHandler;
use systemprompt::agent::services::ArtifactPublishingService;
use systemprompt::database::DbPool;
use systemprompt::identifiers::McpServerId;
use systemprompt::system::AppContext;

use crate::prompts::AdminPrompts;
use crate::resources::AdminResources;
use crate::services::DiscoveredRole;

#[derive(Clone)]
pub struct AdminServer {
    pub(super) db_pool: DbPool,
    pub(super) service_id: McpServerId,
    pub(super) prompts: Arc<AdminPrompts>,
    pub(super) resources: Arc<AdminResources>,
    pub(super) tool_result_handler: Arc<ToolResultHandler>,
    pub(super) publishing_service: Arc<ArtifactPublishingService>,
    pub(super) tool_schemas: Arc<HashMap<String, serde_json::Value>>,
    pub(super) app_context: Arc<AppContext>,
    pub(super) discovered_roles: Arc<Vec<DiscoveredRole>>,
}

impl AdminServer {
    pub async fn new(
        db_pool: DbPool,
        service_id: McpServerId,
        app_context: Arc<AppContext>,
    ) -> Self {
        let prompts = Arc::new(AdminPrompts::new(db_pool.clone(), service_id.to_string()));
        let resources = Arc::new(AdminResources::new(db_pool.clone(), service_id.to_string()));
        let tool_result_handler = Arc::new(ToolResultHandler::new(db_pool.clone()));
        let publishing_service = Arc::new(ArtifactPublishingService::new(db_pool.clone()));

        let discovered_roles = Self::discover_roles(&app_context).await;
        let role_names: Vec<String> = discovered_roles.iter().map(|r| r.name.clone()).collect();
        let tool_schemas = Self::build_tool_schema_cache(&role_names);

        Self {
            db_pool,
            service_id,
            prompts,
            resources,
            tool_result_handler,
            publishing_service,
            tool_schemas: Arc::new(tool_schemas),
            app_context,
            discovered_roles: Arc::new(discovered_roles),
        }
    }

    async fn discover_roles(app_context: &AppContext) -> Vec<DiscoveredRole> {
        use crate::services::RoleDiscoveryService;

        let extensions_path = std::path::Path::new(&app_context.config().system_path)
            .parent()
            .map(|p| p.join("extensions"))
            .unwrap_or_else(|| std::path::PathBuf::from("extensions"));

        let role_service = RoleDiscoveryService::new(extensions_path);
        role_service
            .discover_all_roles()
            .await
            .unwrap_or_else(|_| crate::services::role_discovery::default_core_roles())
    }

    fn build_tool_schema_cache(role_names: &[String]) -> HashMap<String, serde_json::Value> {
        let mut schemas = HashMap::new();
        let tools = crate::tools::register_tools_with_roles(role_names);

        for tool in tools {
            if let Some(output_schema) = tool.output_schema {
                let schema_value =
                    serde_json::to_value(&*output_schema).unwrap_or_else(|_| serde_json::json!({}));
                schemas.insert(tool.name.to_string(), schema_value);
            }
        }

        schemas
    }

    pub(super) fn get_output_schema_for_tool(&self, tool_name: &str) -> Option<serde_json::Value> {
        self.tool_schemas.get(tool_name).cloned()
    }
}
