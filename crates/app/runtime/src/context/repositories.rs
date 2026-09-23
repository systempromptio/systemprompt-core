//! Repository accessors: the composition root builds each repository once
//! and every consumer reaches it through `AppContext`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_agent::repository::A2ARepositories;
use systemprompt_ai::repository::AiRepositories;
use systemprompt_analytics::repository::AnalyticsRepositories;
use systemprompt_content::repository::ContentRepositories;
use systemprompt_database::ServiceRepository;
use systemprompt_files::FileRepository;
use systemprompt_marketplace::managed::ManagedRepository;
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

    pub const fn ai_repositories(&self) -> &Arc<AiRepositories> {
        &self.data.ai_repositories
    }

    pub const fn analytics_repositories(&self) -> &Arc<AnalyticsRepositories> {
        &self.data.analytics_repositories
    }

    pub const fn file_repository(&self) -> &Arc<FileRepository> {
        &self.data.file_repository
    }

    pub const fn mcp_session_repository(&self) -> &Arc<McpSessionRepository> {
        &self.data.mcp_session_repository
    }

    pub const fn managed_repository(&self) -> &Arc<ManagedRepository> {
        &self.data.managed_repository
    }
}
