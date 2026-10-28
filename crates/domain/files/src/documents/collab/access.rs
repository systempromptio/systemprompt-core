//! The access checks every live-session call starts with.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentEditSessionId, DocumentId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, DocumentAction};

use super::{CollabError, CollabService};
use crate::documents::access::AccessTarget;
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentEditSession};

impl CollabService {
    pub(super) async fn authorized(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        action: DocumentAction,
        trace: &TraceId,
    ) -> Result<Document, CollabError> {
        Ok(self.granted(who, document, action, trace).await?.0)
    }

    pub(super) async fn granted(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        action: DocumentAction,
        trace: &TraceId,
    ) -> Result<(Document, AccessLevel), CollabError> {
        let granted = self
            .documents
            .access()
            .authorize(who, AccessTarget::Document(document.clone()), action, trace)
            .await?;
        let level = granted.level;
        let document = granted
            .document
            .ok_or_else(|| DocumentsError::not_found("document", document))?;
        Ok((document, level))
    }

    pub(super) async fn open_matching(
        &self,
        document: &DocumentId,
        session: &DocumentEditSessionId,
    ) -> Result<DocumentEditSession, CollabError> {
        self.documents
            .repositories()
            .edit_sessions
            .find_open(document)
            .await?
            .filter(|open| open.id == *session)
            .ok_or_else(|| CollabError::SessionClosed {
                document_id: document.clone(),
                session_id: session.clone(),
            })
    }
}
