//! Declared streams as readers see them. Streams are not secret; the items
//! published on them are, and each item's visibility is decided per reader.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::UserId;
use systemprompt_models::notifications::StreamDecl;

use super::NotificationsService;
use crate::error::NotificationsResult;

impl NotificationsService {
    pub async fn streams(&self, _user: &UserId) -> NotificationsResult<Vec<StreamDecl>> {
        Ok(self.repos.streams.list_pooled().await?)
    }
}
