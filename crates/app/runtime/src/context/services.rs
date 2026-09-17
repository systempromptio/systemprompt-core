//! Service accessors: the process-wide governance engine, AI service and
//! artifact ingest the composition root built once.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_security::policy::GovernanceEngine;

use super::AppContext;

impl AppContext {
    #[must_use]
    pub fn governance(&self) -> &GovernanceEngine {
        &self.subsystems.governance
    }

    #[must_use]
    pub fn governance_arc(&self) -> Arc<GovernanceEngine> {
        Arc::clone(&self.subsystems.governance)
    }

    #[must_use]
    pub fn artifact_ingest(&self) -> &systemprompt_mcp::ArtifactIngest {
        &self.subsystems.artifact_ingest
    }

    #[must_use]
    pub fn artifact_ingest_arc(&self) -> Arc<systemprompt_mcp::ArtifactIngest> {
        Arc::clone(&self.subsystems.artifact_ingest)
    }

    #[must_use]
    pub const fn ai_service(&self) -> Option<&Arc<systemprompt_ai::AiService>> {
        self.subsystems.ai_service.as_ref()
    }

    #[must_use]
    pub fn ai_service_arc(&self) -> Option<Arc<systemprompt_ai::AiService>> {
        self.subsystems.ai_service.clone()
    }
}
