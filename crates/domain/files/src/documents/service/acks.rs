//! Idempotent writes: the stored acknowledgement of a write that carried an
//! op id, replayed when the same op id arrives again for the document.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentId, DocumentOpId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::DocumentAction;

use super::DocumentService;
use crate::documents::error::DocumentsError;
use crate::documents::models::StoredWriteAck;

impl DocumentService {
    pub async fn find_write_ack(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        op_id: &DocumentOpId,
        trace: &TraceId,
    ) -> Result<Option<StoredWriteAck>, DocumentsError> {
        self.authorized_document(who, document, DocumentAction::Edit, trace)
            .await?;
        Ok(self.repos.write_acks.find(document, op_id).await?)
    }

    pub async fn record_write_ack(
        &self,
        document: &DocumentId,
        op_id: &DocumentOpId,
        ack: &StoredWriteAck,
    ) -> Result<(), DocumentsError> {
        Ok(self.repos.write_acks.insert(document, op_id, ack).await?)
    }
}
