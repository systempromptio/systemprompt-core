//! Client last-use stamping on [`OAuthRepository`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::OAuthRepository;
use crate::error::OauthResult;
use chrono::Utc;
use systemprompt_identifiers::ClientId;

impl OAuthRepository {
    pub async fn update_client_last_used(&self, client_id: &ClientId) -> OauthResult<()> {
        let now = Utc::now().timestamp();
        self.client_repo.update_last_used(client_id, now).await
    }
}
