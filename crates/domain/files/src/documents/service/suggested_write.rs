//! Whole-file writes from outside the store (a bridge Drive commit, a REST
//! byte write) by an agent whose cap on the document is `suggest`: the
//! bytes pass the same upload checks, then land as a whole-body proposal on
//! the client's base (or the head) for a person to review, rather than
//! being refused. Every other write commits as before.
//!
//! A proposal is reviewed before it can change the document, so this path
//! does not need the `replace_content` capability a landed whole-file write
//! does. It counts as one agent edit against the hourly ceiling.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::TraceId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::DocumentAction;

use super::{CommitOutcome, CommitRequest, DocumentService, ProposeBodyRequest, Proposed};
use crate::documents::access::AccessTarget;
use crate::documents::agents::{AgentCap, current_actor};
use crate::documents::error::DocumentsError;
use crate::documents::models::Document;
use crate::documents::revision::CommitBody;
use crate::uploads::Allowance;

/// What an external write did: landed (or conflicted) as a commit, or was
/// written as a proposal because the agent may only suggest.
#[derive(Debug, Clone)]
pub enum ExternalWrite {
    Landed(CommitOutcome),
    Proposed(Box<Proposed>),
}

impl DocumentService {
    /// [`DocumentService::commit_external`], except that an agent capped at
    /// `suggest` writing whole-file bytes gets a proposal instead of
    /// `agent_suggest_only`.
    pub async fn write_external(
        &self,
        who: &AuthenticatedUser,
        request: CommitRequest,
        trace: &TraceId,
    ) -> Result<ExternalWrite, DocumentsError> {
        if let Some(document) = self.suggest_only(who, &request, trace).await? {
            let proposed = self
                .propose_external(who, &document, request, trace)
                .await?;
            return Ok(ExternalWrite::Proposed(Box::new(proposed)));
        }
        Ok(ExternalWrite::Landed(
            self.commit_external(who, request, trace).await?,
        ))
    }

    /// The document, when `request` is an agent's byte write on a document
    /// whose agent cap is `suggest`.
    async fn suggest_only(
        &self,
        who: &AuthenticatedUser,
        request: &CommitRequest,
        trace: &TraceId,
    ) -> Result<Option<Document>, DocumentsError> {
        let bytes = matches!(request.body, CommitBody::Bytes(_));
        if !bytes || !current_actor().is_agent() {
            return Ok(None);
        }
        let granted = self
            .access
            .probe(
                who,
                AccessTarget::Document(request.document_id.clone()),
                DocumentAction::Comment,
                trace,
            )
            .await?;
        let suggest = granted
            .agent_cap
            .as_ref()
            .is_some_and(|cap| cap.cap == AgentCap::Suggest);
        Ok(granted.document.filter(|_| suggest))
    }

    async fn propose_external(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        request: CommitRequest,
        trace: &TraceId,
    ) -> Result<Proposed, DocumentsError> {
        let CommitRequest {
            body,
            base_revision,
            author,
            message,
            ..
        } = request;
        let CommitBody::Bytes(bytes) = body else {
            return Err(DocumentsError::InvalidParams {
                detail: "only a whole-file write is proposed as a body",
            });
        };
        self.check_size(bytes.len())?;
        let (checked, _) = self
            .ingest(
                who,
                &document.name,
                document.format,
                bytes,
                trace,
                Allowance::Save,
            )
            .await?;
        let base = match base_revision {
            Some(base) => base,
            None => self.resolve_revision(document, None).await?.id,
        };
        let reserved = self.reserve_agent_edits(who, 1)?;
        let proposed = self
            .propose_body(
                who,
                ProposeBodyRequest {
                    document_id: document.id.clone(),
                    base_revision: base,
                    bytes: checked,
                    author,
                    message,
                },
                trace,
            )
            .await?;
        reserved.keep();
        Ok(proposed)
    }
}
