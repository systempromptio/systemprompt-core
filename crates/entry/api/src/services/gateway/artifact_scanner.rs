//! The gateway's safety scanners applied to tool-result artifacts.
//!
//! An artifact is scanned with the same scanners the installation's global
//! gateway policy declares, so a tool result is held to the same bar as a
//! model response. The policy is resolved fail-closed: if it cannot be read,
//! the ingest is refused rather than stored unscanned. Every finding is
//! recorded against the artifact; nothing here blocks, because the result
//! has already reached the model — blocking is the gateway's job on the next
//! turn, where the same text is scanned again as request history.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_ai::Finding;
use systemprompt_mcp::{ArtifactFinding, ArtifactScanner, PHASE_TOOL_RESULT};
use systemprompt_models::services::QuotaFaultMode;

use super::policy::PolicyResolver;
use super::protocol::canonical::CanonicalContent;
use super::protocol::canonical_response::CanonicalResponse;
use super::registry::SafetyScannerRegistry;
use super::service::finalize::safety::dedupe_findings;

pub const GATEWAY_ARTIFACT_SCANNER: &str = "gateway_safety";

#[derive(Debug, Clone)]
pub struct GatewayArtifactScanner {
    resolver: PolicyResolver,
}

impl GatewayArtifactScanner {
    #[must_use]
    pub const fn new(resolver: PolicyResolver) -> Self {
        Self { resolver }
    }
}

#[async_trait]
impl ArtifactScanner for GatewayArtifactScanner {
    fn name(&self) -> &'static str {
        GATEWAY_ARTIFACT_SCANNER
    }

    async fn scan(&self, surfaces: &[(String, String)]) -> Result<Vec<ArtifactFinding>, String> {
        let policy = self
            .resolver
            .resolve(QuotaFaultMode::Closed)
            .await
            .map_err(|e| e.to_string())?;
        if policy.safety.scanners.is_empty() || surfaces.is_empty() {
            return Ok(Vec::new());
        }

        let response = CanonicalResponse {
            id: "artifact".to_owned(),
            model: "artifact".to_owned(),
            content: surfaces
                .iter()
                .map(|(_, text)| CanonicalContent::text(text.clone()))
                .collect(),
            stop_reason: None,
            usage: Default::default(),
            grounding: None,
            code_execution: None,
            raw_finish_reason: None,
            received_surface: Default::default(),
        };

        let registry = SafetyScannerRegistry::global();
        let mut findings: Vec<Finding> = Vec::new();
        for name in &policy.safety.scanners {
            match registry.create(name, &policy.safety) {
                Some(scanner) => findings.extend(scanner.scan_response_final(&response).await),
                None => {
                    tracing::warn!(scanner = %name, "Unknown safety scanner in policy — skipped for artifact");
                },
            }
        }
        dedupe_findings(&mut findings);

        Ok(findings
            .into_iter()
            .map(|f| ArtifactFinding {
                phase: PHASE_TOOL_RESULT,
                severity: f.severity.as_str().to_owned(),
                category: f.category,
                scanner: f.scanner.to_owned(),
                path: None,
                excerpt: f.excerpt,
                redacted: false,
            })
            .collect())
    }
}
