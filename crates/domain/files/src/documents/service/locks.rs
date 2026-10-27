//! Edit locks and presence.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_identifiers::{DocumentId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    DocumentAction, DocumentLockEvent, DocumentPresenceEvent, PresenceBeacon,
};

use super::DocumentService;
use crate::documents::error::DocumentsError;
use crate::documents::models::{
    Document, DocumentPresenceRow, LockHolderKind, LockOutcome, LockRequest, PresenceUpsert,
};

impl DocumentService {
    pub async fn acquire_lock(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        holder_kind: LockHolderKind,
        holder_id: &str,
        ttl: Duration,
        trace: &TraceId,
    ) -> Result<LockOutcome, DocumentsError> {
        let document = self
            .authorized_document(who, document, DocumentAction::Edit, trace)
            .await?;
        let outcome = self
            .repos
            .locks
            .acquire(&LockRequest {
                document_id: document.id.clone(),
                holder_user_id: who.id.clone(),
                holder_kind,
                holder_id: holder_id.to_owned(),
                ttl,
            })
            .await?;
        if matches!(outcome, LockOutcome::Acquired(_)) {
            self.announce_lock(who, &document, true).await;
        }
        Ok(outcome)
    }

    pub async fn release_lock(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        holder_id: &str,
        trace: &TraceId,
    ) -> Result<bool, DocumentsError> {
        let document = self
            .authorized_document(who, document, DocumentAction::Edit, trace)
            .await?;
        let released = self.repos.locks.release(&document.id, holder_id).await?;
        if released {
            self.announce_lock(who, &document, false).await;
        }
        Ok(released)
    }

    async fn announce_lock(&self, who: &AuthenticatedUser, document: &Document, locked: bool) {
        let event = DocumentLockEvent {
            document_id: document.id.clone(),
            locked,
            holder_id: Some(who.id.clone()),
            edit_session_id: None,
        };
        let to = self.recipients(document).await;
        if let Err(error) = self.events.lock(&to, &event).await {
            tracing::warn!(document_id = %document.id, error = %error, "Lock event not delivered");
        }
    }

    pub async fn touch_presence(
        &self,
        who: &AuthenticatedUser,
        beacon: PresenceBeacon,
        trace: &TraceId,
    ) -> Result<(), DocumentsError> {
        let document = self
            .authorized_document(who, &beacon.document_id, DocumentAction::View, trace)
            .await?;
        self.repos
            .presence
            .upsert(
                &PresenceUpsert {
                    document_id: document.id.clone(),
                    user_id: who.id.clone(),
                    source: beacon.source_kind,
                    state: beacon.state,
                    cursor: beacon.cursor,
                },
                Duration::from_secs(u64::from(self.config.presence_ttl_secs)),
            )
            .await?;
        let others: Vec<_> = self
            .recipients(&document)
            .await
            .into_iter()
            .filter(|user| *user != who.id)
            .collect();
        let event = DocumentPresenceEvent {
            document_id: document.id.clone(),
            user_id: who.id.clone(),
            state: beacon.state,
            source_kind: beacon.source_kind,
            source_ref: beacon.source_ref,
        };
        if let Err(error) = self.events.presence(&others, &event).await {
            tracing::warn!(
                document_id = %document.id,
                error = %error,
                "Presence event not delivered"
            );
        }
        Ok(())
    }

    pub async fn presence(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        trace: &TraceId,
    ) -> Result<Vec<DocumentPresenceRow>, DocumentsError> {
        let document = self
            .authorized_document(who, document, DocumentAction::View, trace)
            .await?;
        Ok(self.repos.presence.list_active(&document.id).await?)
    }
}
