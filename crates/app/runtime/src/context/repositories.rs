//! Repository accessors: the composition root builds each repository once
//! and every consumer reaches it through `AppContext`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::{CoordinationRepositories, ServiceRepository};
use systemprompt_users::UserRepository;

use super::AppContext;

impl AppContext {
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
