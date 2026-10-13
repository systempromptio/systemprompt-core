//! Composition root for the repositories the SSO driver holds, so the driver
//! and the core audit sink are handed constructed repositories rather than
//! building their own from a pool.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_database::DbPool;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_users::SsoLoginEventRepository;

#[derive(Debug, Clone)]
pub(crate) struct SsoRepositories {
    pub(crate) oauth: OAuthRepository,
    pub(crate) login_events: SsoLoginEventRepository,
}

impl SsoRepositories {
    pub(crate) fn new(db: &DbPool) -> Self {
        Self {
            oauth: OAuthRepository::new(db),
            login_events: SsoLoginEventRepository::new(db),
        }
    }
}
