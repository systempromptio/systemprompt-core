//! Repository bundle for the artifact ingest service.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;

use super::{
    ArtifactFindingRepository, ArtifactPayloadRepository, McpArtifactRepository,
    ToolUsageRepository,
};
use crate::error::McpDomainResult;

#[derive(Debug)]
pub struct ArtifactIngestRepositories {
    pub artifacts: Arc<McpArtifactRepository>,
    pub payloads: Arc<ArtifactPayloadRepository>,
    pub findings: Arc<ArtifactFindingRepository>,
    pub executions: Arc<ToolUsageRepository>,
}

impl ArtifactIngestRepositories {
    pub fn new(db: &DbPool) -> McpDomainResult<Self> {
        Ok(Self {
            artifacts: Arc::new(McpArtifactRepository::new(db)?),
            payloads: Arc::new(ArtifactPayloadRepository::new(db)?),
            findings: Arc::new(ArtifactFindingRepository::new(db)?),
            executions: Arc::new(ToolUsageRepository::new(db)?),
        })
    }
}
