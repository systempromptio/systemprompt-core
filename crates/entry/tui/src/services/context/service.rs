use anyhow::Result;
use serde::{Deserialize, Serialize};

use systemprompt_identifiers::{ContextId, SessionToken};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserContext {
    pub context_id: String,
    pub name: String,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct CreateContextRequest {
    name: String,
}

#[derive(Debug)]
pub struct ContextService {
    api_base: String,
    auth_token: SessionToken,
    client: reqwest::Client,
}

impl ContextService {
    pub fn new(api_base: &str, auth_token: &SessionToken) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;

        Ok(Self {
            api_base: api_base.trim_end_matches('/').to_string(),
            auth_token: auth_token.clone(),
            client,
        })
    }

    pub async fn get_or_create_context(&self) -> Result<ContextId> {
        let contexts = self.list_contexts().await?;

        if let Some(ctx) = contexts.first() {
            tracing::info!("Using existing context: {} ({})", ctx.context_id, ctx.name);
            return Ok(ctx.context_id.clone().into());
        }

        tracing::info!("No existing contexts found, creating new one");
        let new_ctx = self.create_context("TUI Session").await?;
        tracing::info!(
            "Created new context: {} ({})",
            new_ctx.context_id,
            new_ctx.name
        );
        Ok(new_ctx.context_id.into())
    }

    pub async fn force_create_context(&self, name: &str) -> Result<ContextId> {
        tracing::info!("Force creating new context: {}", name);
        let new_ctx = self.create_context(name).await?;
        tracing::info!(
            "Created new context: {} ({})",
            new_ctx.context_id,
            new_ctx.name
        );
        Ok(new_ctx.context_id.into())
    }

    async fn list_contexts(&self) -> Result<Vec<UserContext>> {
        let url = format!("{}/api/v1/core/contexts", self.api_base);
        tracing::debug!("Fetching contexts from: {}", url);

        let response = self.send_get_request(&url).await?;
        Self::ensure_success(&response, "list contexts")?;

        let contexts: Vec<UserContext> = response.json().await?;
        tracing::debug!("Found {} existing contexts", contexts.len());
        Ok(contexts)
    }

    async fn create_context(&self, name: &str) -> Result<UserContext> {
        let url = format!("{}/api/v1/core/contexts", self.api_base);
        tracing::debug!("Creating context at: {}", url);

        let request = CreateContextRequest {
            name: name.to_string(),
        };
        let response = self.send_post_request(&url, &request).await?;
        Self::ensure_success(&response, "create context")?;

        response.json().await.map_err(Into::into)
    }

    async fn send_get_request(&self, url: &str) -> Result<reqwest::Response> {
        self.client
            .get(url)
            .header(
                "Authorization",
                format!("Bearer {}", self.auth_token.as_str()),
            )
            .send()
            .await
            .map_err(Into::into)
    }

    async fn send_post_request<T: Serialize + Sync>(
        &self,
        url: &str,
        body: &T,
    ) -> Result<reqwest::Response> {
        self.client
            .post(url)
            .header(
                "Authorization",
                format!("Bearer {}", self.auth_token.as_str()),
            )
            .json(body)
            .send()
            .await
            .map_err(Into::into)
    }

    fn ensure_success(response: &reqwest::Response, operation: &str) -> Result<()> {
        if response.status().is_success() {
            return Ok(());
        }
        let status = response.status();
        tracing::error!("Failed to {}: HTTP {}", operation, status);
        Err(anyhow::anyhow!("Failed to {}: HTTP {}", operation, status))
    }
}

pub fn create_context_service(
    api_url: &str,
    session_token: &SessionToken,
) -> Result<ContextService> {
    ContextService::new(api_url, session_token)
}
