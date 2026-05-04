use crate::error::{ClientError, ClientResult};
use crate::http;
use chrono::Utc;
use reqwest::Client;
use std::time::Duration;
use systemprompt_identifiers::{ContextId, JwtToken};
use systemprompt_models::a2a::{Task, methods};
use systemprompt_models::admin::{AnalyticsData, LogEntry, UserInfo};
use systemprompt_models::net::{
    HTTP_AUTH_VERIFY_TIMEOUT, HTTP_DEFAULT_TIMEOUT, HTTP_HEALTH_CHECK_TIMEOUT,
};
use systemprompt_models::{
    AgentCard, ApiPaths, CollectionResponse, CreateContextRequest, SingleResponse, UserContext,
    UserContextWithStats,
};

/// Typed HTTP client for a systemprompt.io deployment.
///
/// Holds a pre-built [`reqwest::Client`] (with the workspace default timeout),
/// a base URL with any trailing slash trimmed, and an optional [`JwtToken`].
/// Cheap to clone — the underlying connection pool is shared.
#[derive(Debug, Clone)]
pub struct SystempromptClient {
    base_url: String,
    token: Option<JwtToken>,
    client: Client,
}

impl SystempromptClient {
    /// Build a client against `base_url` using
    /// `systemprompt_models::net::HTTP_DEFAULT_TIMEOUT`.
    pub fn new(base_url: &str) -> ClientResult<Self> {
        let client = Client::builder().timeout(HTTP_DEFAULT_TIMEOUT).build()?;

        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            token: None,
            client,
        })
    }

    /// Build a client with an explicit total-request timeout in seconds.
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

    /// Builder-style setter that attaches a bearer token.
    #[must_use]
    pub fn with_token(mut self, token: JwtToken) -> Self {
        self.token = Some(token);
        self
    }

    /// Replace the bearer token in-place.
    pub fn set_token(&mut self, token: JwtToken) {
        self.token = Some(token);
    }

    /// Borrow the currently configured bearer token, if any.
    #[must_use]
    pub const fn token(&self) -> Option<&JwtToken> {
        self.token.as_ref()
    }

    /// Borrow the configured base URL (with any trailing slash trimmed).
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// `GET` the agent registry and return the agent cards.
    pub async fn list_agents(&self) -> ClientResult<Vec<AgentCard>> {
        let url = format!("{}{}", self.base_url, ApiPaths::AGENTS_REGISTRY);
        let response: CollectionResponse<AgentCard> =
            http::get(&self.client, &url, self.token.as_ref()).await?;
        Ok(response.data)
    }

    /// `GET` an agent's `.well-known/agent-card.json` by name.
    pub async fn get_agent_card(&self, agent_name: &str) -> ClientResult<AgentCard> {
        let url = format!(
            "{}{}",
            self.base_url,
            ApiPaths::wellknown_agent_card_named(agent_name)
        );
        http::get(&self.client, &url, self.token.as_ref()).await
    }

    /// `GET` every context owned by the authenticated user, sorted most-recent
    /// first.
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

    /// `GET` a single context by id.
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

    /// `POST` a new context with an optional human-readable name.
    pub async fn create_context(&self, name: Option<&str>) -> ClientResult<UserContext> {
        let url = format!("{}{}", self.base_url, ApiPaths::CORE_CONTEXTS);
        let request = CreateContextRequest {
            name: name.map(String::from),
        };
        let response: SingleResponse<UserContext> =
            http::post(&self.client, &url, &request, self.token.as_ref()).await?;
        Ok(response.data)
    }

    /// Convenience wrapper that names the new context `"Session YYYY-MM-DD
    /// HH:MM"`.
    pub async fn create_context_auto_name(&self) -> ClientResult<UserContext> {
        let name = format!("Session {}", Utc::now().format("%Y-%m-%d %H:%M"));
        self.create_context(Some(&name)).await
    }

    /// Return the most recent context's id, creating an auto-named context
    /// first when the user has none.
    pub async fn fetch_or_create_context(&self) -> ClientResult<ContextId> {
        let contexts = self.list_contexts().await?;
        if let Some(ctx) = contexts.first() {
            return Ok(ctx.context_id.clone());
        }
        let context = self.create_context_auto_name().await?;
        Ok(context.context_id)
    }

    /// `PUT` a new display name onto an existing context.
    pub async fn update_context_name(&self, context_id: &str, name: &str) -> ClientResult<()> {
        let url = format!(
            "{}{}/{}",
            self.base_url,
            ApiPaths::CORE_CONTEXTS,
            context_id
        );
        let body = serde_json::json!({ "name": name });
        http::put(&self.client, &url, &body, self.token.as_ref()).await
    }

    /// `DELETE` a context (and its tasks/artifacts) by id.
    pub async fn delete_context(&self, context_id: &str) -> ClientResult<()> {
        let url = format!(
            "{}{}/{}",
            self.base_url,
            ApiPaths::CORE_CONTEXTS,
            context_id
        );
        http::delete(&self.client, &url, self.token.as_ref()).await
    }

    /// `GET` every task attached to `context_id`.
    pub async fn list_tasks(&self, context_id: &str) -> ClientResult<Vec<Task>> {
        let url = format!(
            "{}{}/{}/tasks",
            self.base_url,
            ApiPaths::CORE_CONTEXTS,
            context_id
        );
        http::get(&self.client, &url, self.token.as_ref()).await
    }

    /// `DELETE` a task by id.
    pub async fn delete_task(&self, task_id: &str) -> ClientResult<()> {
        let url = format!("{}{}/{}", self.base_url, ApiPaths::CORE_TASKS, task_id);
        http::delete(&self.client, &url, self.token.as_ref()).await
    }

    /// `GET` artifacts for a single context.
    pub async fn list_artifacts(&self, context_id: &str) -> ClientResult<Vec<serde_json::Value>> {
        let url = format!(
            "{}{}/{}/artifacts",
            self.base_url,
            ApiPaths::CORE_CONTEXTS,
            context_id
        );
        http::get(&self.client, &url, self.token.as_ref()).await
    }

    /// Probe the deployment's `/health` endpoint with a short timeout.
    /// Returns `true` iff the server responded at all (any status).
    pub async fn check_health(&self) -> bool {
        let url = format!("{}{}", self.base_url, ApiPaths::HEALTH);
        self.client
            .get(&url)
            .timeout(HTTP_HEALTH_CHECK_TIMEOUT)
            .send()
            .await
            .is_ok()
    }

    /// Verify the configured bearer token by hitting `/auth/me`. Returns
    /// [`ClientError::AuthError`] when no token is configured.
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

    /// Send an A2A `message/send` JSON-RPC call to the named agent.
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

    /// `GET /admin/logs`, optionally bounded by `limit`. Requires admin auth.
    pub async fn list_logs(&self, limit: Option<u32>) -> ClientResult<Vec<LogEntry>> {
        let url = limit.map_or_else(
            || format!("{}{}", self.base_url, ApiPaths::ADMIN_LOGS),
            |l| format!("{}{}?limit={}", self.base_url, ApiPaths::ADMIN_LOGS, l),
        );
        http::get(&self.client, &url, self.token.as_ref()).await
    }

    /// `GET /admin/users`, optionally bounded by `limit`. Requires admin auth.
    pub async fn list_users(&self, limit: Option<u32>) -> ClientResult<Vec<UserInfo>> {
        let url = limit.map_or_else(
            || format!("{}{}", self.base_url, ApiPaths::ADMIN_USERS),
            |l| format!("{}{}?limit={}", self.base_url, ApiPaths::ADMIN_USERS, l),
        );
        http::get(&self.client, &url, self.token.as_ref()).await
    }

    /// `GET /admin/analytics`. Requires admin auth.
    pub async fn get_analytics(&self) -> ClientResult<AnalyticsData> {
        let url = format!("{}{}", self.base_url, ApiPaths::ADMIN_ANALYTICS);
        http::get(&self.client, &url, self.token.as_ref()).await
    }

    /// `GET /artifacts` across all contexts owned by the user, optionally
    /// bounded by `limit`.
    pub async fn list_all_artifacts(
        &self,
        limit: Option<u32>,
    ) -> ClientResult<Vec<serde_json::Value>> {
        let url = limit.map_or_else(
            || format!("{}{}", self.base_url, ApiPaths::CORE_ARTIFACTS),
            |l| format!("{}{}?limit={}", self.base_url, ApiPaths::CORE_ARTIFACTS, l),
        );
        http::get(&self.client, &url, self.token.as_ref()).await
    }

    fn auth_header(&self) -> ClientResult<String> {
        self.token.as_ref().map_or_else(
            || Err(ClientError::AuthError("No token configured".to_string())),
            |token| Ok(format!("Bearer {}", token.as_str())),
        )
    }
}
