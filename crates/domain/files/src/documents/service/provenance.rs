//! Whether a document carries agent content: the flag the rendered view
//! picks its tier by, for a caller already authorised to view the document,
//! and how a landing sets it — when an agent wrote the revision or the
//! revision settles a proposal.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentId, DocumentProposalId};
use systemprompt_models::documents::ActorKind;

use super::DocumentService;
use crate::documents::error::DocumentsError;
use crate::documents::repository::DocumentRepository;

pub(super) async fn mark_agent_content(
    tx: &mut sqlx::PgConnection,
    document: &DocumentId,
    authored: bool,
    settles: Option<&DocumentProposalId>,
) -> Result<(), DocumentsError> {
    if authored || settles.is_some() {
        DocumentRepository::mark_agent_content_in(tx, document, authored, settles).await?;
    }
    Ok(())
}

impl DocumentService {
    pub async fn agent_content(&self, document: &DocumentId) -> Result<bool, DocumentsError> {
        Ok(self.repos.documents.agent_content(document).await?)
    }

    pub(super) fn agent_wrote(&self, actor_kind: ActorKind) -> bool {
        actor_kind == ActorKind::Agent || self.access.actor().as_agent().is_some()
    }
}
