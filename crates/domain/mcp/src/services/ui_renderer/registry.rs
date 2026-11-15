//! Registry of `UiRenderer` implementations keyed by artifact type.
//!
//! [`UiRendererRegistry::with_builtins`] holds core's renderers. The built
//! [`UiSlot`](crate::contract::UiSlot) is this registry with every contributed
//! renderer folded in (a contributed renderer for an artifact type that has a
//! built-in replaces it) and the contributed theme, which [`render`]
//! applies to every document.
//!
//! [`render`]: UiRendererRegistry::render
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{ArtifactTheme, UiRenderer, UiResource};
use crate::error::{McpDomainError, McpDomainResult};
use std::collections::HashMap;
use std::sync::Arc;
use systemprompt_models::a2a::Artifact;
use systemprompt_models::artifacts::CliArtifact;

/// Renderers keyed by artifact type, and the theme applied to their output.
#[derive(Default)]
pub struct UiRendererRegistry {
    renderers: HashMap<String, Arc<dyn UiRenderer>>,
    theme: Option<ArtifactTheme>,
}

impl UiRendererRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        registry.register(super::templates::TableRenderer::new());
        registry.register(super::templates::ChartRenderer::new());
        registry.register(super::templates::TextRenderer::new());
        registry.register(super::templates::CopyPasteTextRenderer::new());
        registry.register(super::templates::FormRenderer::new());
        registry.register(super::templates::ListRenderer::new());
        registry.register(super::templates::ImageRenderer::new());
        registry.register(super::templates::AudioRenderer::new());
        registry.register(super::templates::VideoRenderer::new());
        registry.register(super::templates::DashboardRenderer::new());
        registry.register(super::templates::PresentationCardRenderer::new());
        registry.register(super::templates::MessageRenderer::new());
        registry
    }

    pub const fn set_theme(&mut self, theme: ArtifactTheme) {
        self.theme = Some(theme);
    }

    pub const fn theme(&self) -> Option<&ArtifactTheme> {
        self.theme.as_ref()
    }

    pub fn register<R: UiRenderer + 'static>(&mut self, renderer: R) {
        self.register_arc(Arc::new(renderer));
    }

    pub fn register_arc(&mut self, renderer: Arc<dyn UiRenderer>) {
        let artifact_type = renderer.artifact_type().to_string();
        self.renderers.insert(artifact_type, renderer);
    }

    pub fn get(&self, artifact_type: &str) -> Option<Arc<dyn UiRenderer>> {
        self.renderers.get(artifact_type).cloned()
    }

    pub fn supports(&self, artifact_type: &str) -> bool {
        self.renderers.contains_key(artifact_type)
    }

    pub fn supported_types(&self) -> Vec<&str> {
        self.renderers.keys().map(String::as_str).collect()
    }

    pub fn render(&self, artifact: &Artifact) -> McpDomainResult<UiResource> {
        let artifact_type = resolve_artifact_type(artifact);

        let renderer =
            self.get(artifact_type)
                .ok_or_else(|| McpDomainError::NoRendererForArtifactType {
                    artifact_type: artifact_type.to_owned(),
                })?;

        let resource = renderer.render(artifact)?;
        Ok(match &self.theme {
            Some(theme) => UiResource {
                html: theme.apply(&resource.html),
                csp: resource.csp,
            },
            None => resource,
        })
    }
}

pub fn resolve_artifact_type(artifact: &Artifact) -> &str {
    let declared = artifact.metadata.artifact_type.as_str();
    if declared != CliArtifact::ENVELOPE_TYPE_STR {
        return declared;
    }

    artifact
        .parts
        .iter()
        .find_map(|part| {
            let data = part.as_data()?;
            data.get("x-artifact-type")
                .or_else(|| data.get("artifact_type"))
                .and_then(serde_json::Value::as_str)
        })
        .unwrap_or(declared)
}

impl std::fmt::Debug for UiRendererRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UiRendererRegistry")
            .field("registered_types", &self.supported_types())
            .field("themed", &self.theme.is_some())
            .finish()
    }
}
