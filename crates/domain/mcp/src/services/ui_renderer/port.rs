//! [`UiRendererRegistry`] as the shared [`ArtifactUiRenderer`] contract.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::artifact_ui::{ArtifactUiError, ArtifactUiRenderer, RenderedArtifactUi};
use systemprompt_models::a2a::Artifact;

use super::UiRendererRegistry;
use super::registry::resolve_artifact_type;

impl ArtifactUiRenderer for UiRendererRegistry {
    fn render(&self, artifact: &Artifact) -> Result<RenderedArtifactUi, ArtifactUiError> {
        let artifact_type = resolve_artifact_type(artifact);
        if !self.supports(artifact_type) {
            return Err(ArtifactUiError::Unsupported {
                artifact_type: artifact_type.to_owned(),
            });
        }
        let resource = Self::render(self, artifact).map_err(|e| ArtifactUiError::Render(Box::new(e)))?;
        Ok(RenderedArtifactUi {
            content_security_policy: resource.csp.to_header_value(),
            html: resource.html,
        })
    }
}
