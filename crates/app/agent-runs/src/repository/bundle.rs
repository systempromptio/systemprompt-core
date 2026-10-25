//! [`AgentRunRepositories`]: every repository a run's claim loop and its
//! credential service use, built once from the pool and injected.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_oauth::OAuthRepository;
use systemprompt_users::{SessionRepository, UserRepository};

use super::AgentRunRepository;

/// The repositories [`crate::credentials::RunCredentialService`] issues and
/// revokes a run's PAT, gateway session and hook token through.
#[derive(Debug, Clone)]
pub struct RunCredentialRepositories {
    pub users: Arc<UserRepository>,
    pub sessions: SessionRepository,
    pub oauth: OAuthRepository,
}

impl RunCredentialRepositories {
    #[must_use]
    pub fn new(db: &DbPool, oauth: OAuthRepository) -> Self {
        Self {
            users: Arc::new(UserRepository::new(db)),
            sessions: SessionRepository::new(db),
            oauth,
        }
    }
}

/// The run rows plus the credential repositories, for a runner service.
#[derive(Debug, Clone)]
pub struct AgentRunRepositories {
    pub runs: AgentRunRepository,
    pub credentials: RunCredentialRepositories,
    pub managed: systemprompt_marketplace::managed::ManagedRepository,
}

impl AgentRunRepositories {
    #[must_use]
    pub fn new(db: &DbPool, oauth: OAuthRepository) -> Self {
        Self {
            runs: AgentRunRepository::new(db),
            credentials: RunCredentialRepositories::new(db, oauth),
            managed: systemprompt_marketplace::managed::ManagedRepository::new(db),
        }
    }
}
