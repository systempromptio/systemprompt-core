//! Repository accessors: the composition root builds each repository once
//! and every consumer reaches it through `AppContext`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_content::repository::ContentRepositories;
use systemprompt_database::{CoordinationRepositories, ServiceRepository};
use systemprompt_oauth::repository::OAuthRepositories;
use systemprompt_users::UserRepository;

use super::AppContext;

impl AppContext {
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
}
