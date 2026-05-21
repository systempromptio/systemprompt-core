use crate::error::{ClientError, ClientResult};
use crate::http;
use chrono::Utc;
use reqwest::Client;
use std::time::Duration;
use systemprompt_identifiers::{ContextId, JwtToken, TaskId};
use systemprompt_models::a2a::{Task, methods};
use systemprompt_models::admin::{AnalyticsData, LogEntry, UserInfo};
use systemprompt_models::net::{
    HTTP_AUTH_VERIFY_TIMEOUT, HTTP_DEFAULT_TIMEOUT, HTTP_HEALTH_CHECK_TIMEOUT,
};
use systemprompt_models::{
    AgentCard, ApiPaths, CollectionResponse, CreateContextRequest, SingleResponse, UserContext,
    UserContextWithStats,
};

#[derive(Debug, Clone)]
pub struct SystempromptClient {
    base_url: String,
    token: Option<JwtToken>,
    client: Client,
}

impl SystempromptClient {
    pub fn new(base_url: &str) -> ClientResult<Self> {
        let client = Client::builder().timeout(HTTP_DEFAULT_TIMEOUT).build()?;

        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            token: None,
            client,
        })
    }

    pub fn with_timeout(base_url: &str, timeout_secs: u64) -> ClientResult<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()?;

        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            token: None,
            client,
        })
    }

    #[must_use]
    pub fn with_token(mut self, token: JwtToken) -> Self {
        self.token = Some(token);
        self
    }

    pub fn set_token(&mut self, token: JwtToken) {
        self.token = Some(token);
    }

    #[must_use]
    pub const fn token(&self) -> Option<&JwtToken> {
        self.token.as_ref()
    }

    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn list_agents(&self) -> ClientResult<Vec<AgentCard>> {
        let url = format!("{}{}", self.base_url, ApiPaths::AGENTS_REGISTRY);
        let response: CollectionResponse<AgentCard> =
            http::get(&self.client, &url, self.token.as_ref()).await?;
        Ok(response.data)
    }

    pub async fn get_agent_card(&self, agent_name: &str) -> ClientResult<AgentCard> {
        let url = format!(
            "{}{}",
            self.base_url,
            ApiPaths::wellknown_agent_card_named(agent_name)
        );
        http::get(&self.client, &url, self.token.as_ref()).await
    }

    pub async fn list_contexts(&self) -> ClientResult<Vec<UserContextWithStats>> {
        let url = format!(
            "{}{}?sort=updated_at:desc",
            self.base_url,
            ApiPaths::CORE_CONTEXTS
        );
        let response: CollectionResponse<UserContextWithStats> =
            http::get(&self.client, &url, self.token.as_ref()).await?;
        Ok(response.data)
    }

    pub async fn get_context(&self, context_id: &ContextId) -> ClientResult<UserContext> {
        let url = format!(
            "{}{}/{}",
            self.base_url,
            ApiPaths::CORE_CONTEXTS,
            context_id.as_ref()
        );
        let response: SingleResponse<UserContext> =
            http::get(&self.client, &url, self.token.as_ref()).await?;
        Ok(response.data)
    }

    pub async fn create_context(&self, name: Option<&str>) -> ClientResult<UserContext> {
        let url = format!("{}{}", self.base_url, ApiPaths::CORE_CONTEXTS);
        let request = CreateContextRequest {
            name: name.map(String::from),
        };
        let response: SingleResponse<UserContext> =
            http::post(&self.client, &url, &request, self.token.as_ref()).await?;
        Ok(response.data)
    }

    pub async fn create_context_auto_name(&self) -> ClientResult<UserContext> {
        let name = format!("Session {}", Utc::now().format("%Y-%m-%d %H:%M"));
        self.create_context(Some(&name)).await
    }

    pub async fn fetch_or_create_context(&self) -> ClientResult<ContextId> {
        let contexts = self.list_contexts().await?;
        if let Some(ctx) = contexts.first() {
            return Ok(ctx.context_id.clone());
        }
        let context = self.create_context_auto_name().await?;
        Ok(context.context_id)
    }

    pub async fn update_context_name(
        &self,
        context_id: &ContextId,
        name: &str,
    ) -> ClientResult<()> {
        let url = format!(
            "{}{}/{}",
            self.base_url,
            ApiPaths::CORE_CONTEXTS,
            context_id.as_str()
        );
        let body = serde_json::json!({ "name": name });
        http::put(&self.client, &url, &body, self.token.as_ref()).await
    }

    pub async fn delete_context(&self, context_id: &ContextId) -> ClientResult<()> {
        let url = format!(
            "{}{}/{}",
            self.base_url,
            ApiPaths::CORE_CONTEXTS,
            context_id.as_str()
        );
        http::delete(&self.client, &url, self.token.as_ref()).await
    }

    pub async fn list_tasks(&self, context_id: &ContextId) -> ClientResult<Vec<Task>> {
        let url = format!(
            "{}{}/{}/tasks",
            self.base_url,
            ApiPaths::CORE_CONTEXTS,
            context_id.as_str()
        );
        http::get(&self.client, &url, self.token.as_ref()).await
    }

    pub async fn delete_task(&self, task_id: &TaskId) -> ClientResult<()> {
        let url = format!(
            "{}{}/{}",
            self.base_url,
            ApiPaths::CORE_TASKS,
            task_id.as_str()
        );
        http::delete(&self.client, &url, self.token.as_ref()).await
    }

    // JSON: HTTP boundary. The shared client does not depend on the agent
    // crate, so artifact rows are surfaced as raw JSON; callers that need
    // typed access deserialize into `systemprompt_models::a2a::Artifact`.
    pub async fn list_artifacts(
        &self,
        context_id: &ContextId,
    ) -> ClientResult<Vec<serde_json::Value>> {
        let url = format!(
            "{}{}/{}/artifacts",
            self.base_url,
            ApiPaths::CORE_CONTEXTS,
            context_id.as_str()
        );
        http::get(&self.client, &url, self.token.as_ref()).await
    }

    pub async fn check_health(&self) -> bool {
        let url = format!("{}{}", self.base_url, ApiPaths::HEALTH);
        self.client
            .get(&url)
            .timeout(HTTP_HEALTH_CHECK_TIMEOUT)
            .send()
            .await
            .is_ok()
    }

    pub async fn verify_token(&self) -> ClientResult<bool> {
        let url = format!("{}{}", self.base_url, ApiPaths::AUTH_ME);
        let auth = self.auth_header()?;
        let response = self
            .client
            .get(&url)
            .timeout(HTTP_AUTH_VERIFY_TIMEOUT)
            .header("Authorization", auth)
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    // JSON: A2A JSON-RPC 2.0 envelope. Both the inbound `message` and the
    // returned response object are passed through as raw JSON so the shared
    // client stays free of the agent-domain dependency.
    pub async fn send_message(
        &self,
        agent_name: &str,
        context_id: &ContextId,
        message: serde_json::Value,
    ) -> ClientResult<serde_json::Value> {
        let url = format!("{}{}/{}/", self.base_url, ApiPaths::AGENTS_BASE, agent_name);
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": methods::SEND_MESSAGE,
            "params": { "message": message },
            "id": context_id.as_ref()
        });
        http::post(&self.client, &url, &request, self.token.as_ref()).await
    }

    fn limited_url(&self, path: &str, limit: Option<u32>) -> String {
        limit.map_or_else(
            || format!("{}{}", self.base_url, path),
            |l| format!("{}{}?limit={}", self.base_url, path, l),
        )
    }

    pub async fn list_logs(&self, limit: Option<u32>) -> ClientResult<Vec<LogEntry>> {
        let url = self.limited_url(ApiPaths::ADMIN_LOGS, limit);
        http::get(&self.client, &url, self.token.as_ref()).await
    }

    pub async fn list_users(&self, limit: Option<u32>) -> ClientResult<Vec<UserInfo>> {
        let url = self.limited_url(ApiPaths::ADMIN_USERS, limit);
        http::get(&self.client, &url, self.token.as_ref()).await
    }

    pub async fn get_analytics(&self) -> ClientResult<AnalyticsData> {
        let url = format!("{}{}", self.base_url, ApiPaths::ADMIN_ANALYTICS);
        http::get(&self.client, &url, self.token.as_ref()).await
    }

    // JSON: HTTP boundary, see `list_artifacts`.
    pub async fn list_all_artifacts(
        &self,
        limit: Option<u32>,
    ) -> ClientResult<Vec<serde_json::Value>> {
        let url = self.limited_url(ApiPaths::CORE_ARTIFACTS, limit);
        http::get(&self.client, &url, self.token.as_ref()).await
    }

    fn auth_header(&self) -> ClientResult<String> {
        self.token.as_ref().map_or_else(
            || Err(ClientError::AuthError("No token configured".to_string())),
            |token| Ok(format!("Bearer {}", token.as_str())),
        )
    }
}
