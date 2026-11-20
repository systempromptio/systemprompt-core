//! Repository accessors: the composition root builds each repository once
//! and every consumer reaches it through `AppContext`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_agent::repository::A2ARepositories;
use systemprompt_ai::repository::AiRepositories;
use systemprompt_content::repository::ContentRepositories;
use systemprompt_database::{CoordinationRepositories, ServiceRepository};
use systemprompt_mcp::repository::McpSessionRepository;
use systemprompt_oauth::repository::OAuthRepositories;
use systemprompt_users::UserRepository;

use super::AppContext;

impl AppContext {
    pub const fn a2a_repositories(&self) -> &Arc<A2ARepositories> {
        &self.data.a2a_repositories
    }

    pub const fn content_repositories(&self) -> &Arc<ContentRepositories> {
        &self.data.content_repositories
    }

    pub const fn oauth_repositories(&self) -> &Arc<OAuthRepositories> {
        &self.data.oauth_repositories
    }

    pub const fn user_repository(&self) -> &Arc<UserRepository> {
        &self.data.user_repository
    }

    pub const fn service_repository(&self) -> &Arc<ServiceRepository> {
        &self.data.service_repository
    }

    pub const fn coordination_repositories(&self) -> &Arc<CoordinationRepositories> {
        &self.data.coordination
    }

    pub const fn ai_repositories(&self) -> &Arc<AiRepositories> {
        &self.data.ai_repositories
    }

    #[must_use]
    pub fn tool_call_intents(&self) -> systemprompt_traits::DynToolCallIntentClaims {
        Arc::new(self.data.ai_repositories.requests.clone())
    }

    pub const fn mcp_session_repository(&self) -> &Arc<McpSessionRepository> {
        &self.data.mcp_session_repository
    }
}
