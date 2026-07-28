//! Registry of `UiRenderer` implementations keyed by artifact type.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{UiRenderer, UiResource};
use crate::error::{McpDomainError, McpDomainResult};
use std::collections::HashMap;
use std::sync::Arc;
use systemprompt_models::a2a::{Artifact, Part};
use systemprompt_models::artifacts::CliArtifact;

pub struct UiRendererRegistry {
    renderers: HashMap<String, Arc<dyn UiRenderer>>,
}

impl Default for UiRendererRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl UiRendererRegistry {
    pub fn new() -> Self {
        Self {
            renderers: HashMap::new(),
        }
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

    pub async fn render(&self, artifact: &Artifact) -> McpDomainResult<UiResource> {
        let artifact_type = resolve_artifact_type(artifact);

        let renderer = self.get(artifact_type).ok_or_else(|| {
            McpDomainError::Internal(format!(
                "No renderer registered for artifact type: {artifact_type}"
            ))
        })?;

        renderer.render(artifact).await
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
        .find_map(|part| match part {
            Part::Data(data) => data
                .data
                .get("artifact_type")
                .or_else(|| data.data.get("x-artifact-type"))
                .and_then(serde_json::Value::as_str),
            Part::Text(_) | Part::File(_) => None,
        })
        .unwrap_or(declared)
}

impl std::fmt::Debug for UiRendererRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UiRendererRegistry")
            .field("registered_types", &self.supported_types())
            .finish()
    }
}

/// Compile-time registration of a [`UiRenderer`] implementation.
///
/// The default registry seeds its built-ins, then folds in every
/// `inventory`-collected registration. A registration for an artifact type that
/// already has a built-in replaces it — that is the supported way to change how
/// one artifact type renders without forking the whole registry.
#[derive(Debug, Clone, Copy)]
pub struct UiRendererRegistration {
    pub name: &'static str,
    pub factory: fn() -> Arc<dyn UiRenderer>,
}

inventory::collect!(UiRendererRegistration);

/// Register a [`UiRenderer`] implementation, replacing the built-in for its
/// artifact type if there is one.
///
/// ```ignore
/// use systemprompt_mcp::register_ui_renderer;
/// register_ui_renderer!(BrandTableRenderer::new, name = "brand-table");
/// ```
///
/// `$factory` is any `fn() -> R where R: UiRenderer + 'static`.
#[macro_export]
macro_rules! register_ui_renderer {
    ($factory:expr, name = $name:expr $(,)?) => {
        ::inventory::submit! {
            $crate::services::ui_renderer::registry::UiRendererRegistration {
                name: $name,
                factory: || ::std::sync::Arc::new($factory()),
            }
        }
    };
}

pub fn create_default_registry() -> UiRendererRegistry {
    let mut registry = UiRendererRegistry::new();

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

    for registration in inventory::iter::<UiRendererRegistration>() {
        let renderer = (registration.factory)();
        let artifact_type = renderer.artifact_type().to_string();
        if registry.supports(&artifact_type) {
            tracing::info!(
                renderer = registration.name,
                artifact_type = %artifact_type,
                "registered UI renderer replaces the built-in"
            );
        }
        registry.register_arc(renderer);
    }

    registry
}
