//! Recording and announcing a checked share: the share notification is
//! written on the grant's own transaction, and the live share events are
//! published once it has committed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;
use systemprompt_models::auth::AuthenticatedUser;

use super::DocumentService;
use super::grants::CheckedShare;
use super::notify::ShareNotice;
use super::ownership::ShareCause;
use crate::documents::error::DocumentsError;
use crate::documents::models::DocumentGrant;

impl DocumentService {
    pub(super) async fn record_checked(
        &self,
        conn: &mut PgConnection,
        who: &AuthenticatedUser,
        checked: &CheckedShare,
        grant: &DocumentGrant,
    ) -> Result<(), DocumentsError> {
        let Some(document) = &checked.granted.document else {
            return Ok(());
        };
        let notice = ShareNotice {
            who,
            document,
            recipients: &checked.recipients,
            level: grant.access_level,
            subject_kind: &grant.subject_kind,
            subject_value: &grant.subject_value,
            message: checked.message.as_deref(),
            via_agent: checked.via_agent.as_deref(),
            occurred_at: grant.updated_at,
        };
        self.record_share(conn, notice).await
    }

    pub(super) async fn announce_checked(
        &self,
        who: &AuthenticatedUser,
        checked: CheckedShare,
        grant: &DocumentGrant,
    ) {
        if let Some(document) = &checked.granted.document {
            let cause = ShareCause {
                via_agent: checked.via_agent,
                comment_id: None,
            };
            self.announce_share(
                who,
                document,
                grant,
                &checked.recipients,
                checked.message,
                &cause,
            )
            .await;
        }
    }
}
