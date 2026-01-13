use anyhow::Result;
use rmcp::{
    model::{
        ListResourcesResult, PaginatedRequestParam, ReadResourceRequestParam, ReadResourceResult,
    },
    service::RequestContext,
    ErrorData as McpError, RoleServer,
};
use systemprompt::database::DbPool;

#[derive(Clone)]
pub struct AdminResources {
    _db_pool: DbPool,
    _server_name: String,
}

impl AdminResources {
    #[must_use]
    pub fn new(db_pool: DbPool, server_name: String) -> Self {
        Self {
            _db_pool: db_pool,
            _server_name: server_name,
        }
    }

    pub async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParam>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult {
            next_cursor: None,
            resources: Vec::new(),
            meta: None,
        })
    }

    pub async fn read_resource(
        &self,
        _request: ReadResourceRequestParam,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        Err(McpError::invalid_params(
            "No resources currently available".to_string(),
            None,
        ))
    }
}
