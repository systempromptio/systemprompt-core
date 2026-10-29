//! Writing, recording and announcing a checked share: the grant is written
//! only if it does not lower an existing grant above the sharer's own level
//! (checked again by the write itself, so a concurrent stronger grant is
//! never downgraded), the share notification is written on the grant's own
//! transaction, and the live share events are published once it has
//! committed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;
use systemprompt_models::auth::AuthenticatedUser;

use super::DocumentService;
use super::grants::{CheckedShare, owner_of};
use super::notify::ShareNotice;
use super::ownership::ShareCause;
use crate::documents::error::DocumentsError;
use crate::documents::models::DocumentGrant;

impl DocumentService {
    pub(super) async fn insert_checked(
        &self,
        conn: &mut PgConnection,
        checked: &CheckedShare,
    ) -> Result<DocumentGrant, DocumentsError> {
        let held = checked.granted.level;
        if let Some(grant) = self
            .repos
            .grants
            .insert_bounded(conn, checked.grant.clone(), held)
            .await?
        {
            return Ok(grant);
        }
        let existing = self
            .list_grants(&checked.grant.target)
            .await?
            .into_iter()
            .find(|grant| {
                grant.subject_kind == checked.grant.subject_kind
                    && grant.subject_value == checked.grant.subject_value
            })
            .map_or(held, |grant| grant.access_level);
        Err(DocumentsError::ShareBelowExisting {
            existing,
            held,
            owner: owner_of(&checked.granted),
        })
    }

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
