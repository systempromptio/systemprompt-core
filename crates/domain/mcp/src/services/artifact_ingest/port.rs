//! [`ArtifactIngest`] as the shared [`ToolResultIngest`] contract another
//! capability hands tool results to.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::mcp::artifacts::{
    ArtifactScanner, IngestMark, IngestMarkKey, IngestOutcome, IngestRequest, ToolResultIngest,
    ToolResultIngestError,
};

use super::ArtifactIngest;
use crate::error::McpDomainError;

fn port_error(error: McpDomainError) -> ToolResultIngestError {
    ToolResultIngestError(Box::new(error))
}

#[async_trait]
impl ToolResultIngest for ArtifactIngest {
    fn register_scanner(&self, scanner: Arc<dyn ArtifactScanner>) {
        Self::register_scanner(self, scanner);
    }

    async fn ingest(&self, request: IngestRequest) -> Result<IngestOutcome, ToolResultIngestError> {
        Self::ingest(self, request).await.map_err(port_error)
    }

    async fn find_mark(&self, key: &IngestMarkKey) -> Result<Option<IngestMark>, ToolResultIngestError> {
        self.ingest_marks().find_mark(key).await.map_err(port_error)
    }

    async fn set_mark(&self, key: &IngestMarkKey, mark: &IngestMark) -> Result<(), ToolResultIngestError> {
        self.ingest_marks().set_mark(key, mark).await.map_err(port_error)
    }

    async fn list_settled_gateway_call_ids(
        &self,
        ids: &[String],
    ) -> Result<Vec<String>, ToolResultIngestError> {
        self.ingest_marks()
            .list_settled_gateway_call_ids(ids)
            .await
            .map_err(port_error)
    }
}
