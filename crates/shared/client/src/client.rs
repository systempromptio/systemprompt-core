use crate::error::{ClientError, ClientResult};
use crate::http;
use chrono::Utc;
use reqwest::Client;
use std::time::Duration;
use systemprompt_identifiers::{ContextId, JwtToken};
use systemprompt_models::a2a::Task;
use systemprompt_models::admin::{AnalyticsData, LogEntry, UserInfo};
use systemprompt_models::{
    AgentCard, ApiPaths, CollectionResponse, CreateContextRequest, SingleResponse, UserContext,
    UserContextWithStats,
};

const DEFAULT_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone)]
pub struct SystempromptClient {
    base_url: String,
    token: Option<JwtToken>,
    client: Client,
}

impl SystempromptClient {
    pub fn new(base_url: &str) -> ClientResult<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .build()?;

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

    pub fn with_token(mut self, token: JwtToken) -> Self {
        self.token = Some(token);
        self
    }

    pub fn set_token(&mut self, token: JwtToken) {
        self.token = Some(token);
    }

    pub const fn token(&self) -> Option<&JwtToken> {
        self.token.as_ref()
    }

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
        let name = format!("TUI Session {}", Utc::now().format("%Y-%m-%d %H:%M"));
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

    pub async fn delete_context(&self, context_id: &str) -> ClientResult<()> {
        let url = format!(
            "{}{}/{}",
            self.base_url,
            ApiPaths::CORE_CONTEXTS,
            context_id
        );
        http::delete(&self.client, &url, self.token.as_ref()).await
    }

    pub async fn list_tasks(&self, context_id: &str) -> ClientResult<Vec<Task>> {
        let url = format!(
            "{}{}/{}/tasks",
            self.base_url,
            ApiPaths::CORE_CONTEXTS,
            context_id
        );
        http::get(&self.client, &url, self.token.as_ref()).await
    }

    pub async fn delete_task(&self, task_id: &str) -> ClientResult<()> {
        let url = format!("{}{}/{}", self.base_url, ApiPaths::CORE_TASKS, task_id);
        http::delete(&self.client, &url, self.token.as_ref()).await
    }

    pub async fn list_artifacts(&self, context_id: &str) -> ClientResult<Vec<serde_json::Value>> {
        let url = format!(
            "{}{}/{}/artifacts",
            self.base_url,
            ApiPaths::CORE_CONTEXTS,
            context_id
        );
        http::get(&self.client, &url, self.token.as_ref()).await
    }

    pub async fn check_health(&self) -> bool {
        let url = format!("{}{}", self.base_url, ApiPaths::HEALTH);
        self.client
            .get(&url)
            .timeout(Duration::from_secs(5))
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
            .timeout(Duration::from_secs(10))
            .header("Authorization", auth)
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    pub async fn send_message(
        &self,
        agent_name: &str,
        context_id: &ContextId,
        message: serde_json::Value,
    ) -> ClientResult<serde_json::Value> {
        let url = format!("{}{}/{}/", self.base_url, ApiPaths::AGENTS_BASE, agent_name);
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "message/send",
            "params": { "message": message },
            "id": context_id.as_ref()
        });
        http::post(&self.client, &url, &request, self.token.as_ref()).await
    }

    pub async fn list_logs(&self, limit: Option<u32>) -> ClientResult<Vec<LogEntry>> {
        let url = limit.map_or_else(
            || format!("{}{}", self.base_url, ApiPaths::ADMIN_LOGS),
            |l| format!("{}{}?limit={}", self.base_url, ApiPaths::ADMIN_LOGS, l),
        );
        http::get(&self.client, &url, self.token.as_ref()).await
    }

    pub async fn list_users(&self, limit: Option<u32>) -> ClientResult<Vec<UserInfo>> {
        let url = limit.map_or_else(
            || format!("{}{}", self.base_url, ApiPaths::ADMIN_USERS),
            |l| format!("{}{}?limit={}", self.base_url, ApiPaths::ADMIN_USERS, l),
        );
        http::get(&self.client, &url, self.token.as_ref()).await
    }

    pub async fn get_analytics(&self) -> ClientResult<AnalyticsData> {
        let url = format!("{}{}", self.base_url, ApiPaths::ADMIN_ANALYTICS);
        http::get(&self.client, &url, self.token.as_ref()).await
    }

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
