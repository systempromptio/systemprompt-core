use super::{UiRenderer, UiResource};
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::Arc;
use systemprompt_models::a2a::Artifact;

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
        let artifact_type = renderer.artifact_type().to_string();
        self.renderers.insert(artifact_type, Arc::new(renderer));
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

    pub async fn render(&self, artifact: &Artifact) -> Result<UiResource> {
        let artifact_type = &artifact.metadata.artifact_type;

        let renderer = self.get(artifact_type).ok_or_else(|| {
            anyhow!(
                "No renderer registered for artifact type: {}",
                artifact_type
            )
        })?;

        renderer.render(artifact).await
    }
}

impl std::fmt::Debug for UiRendererRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UiRendererRegistry")
            .field("registered_types", &self.supported_types())
            .finish()
    }
}

pub fn create_default_registry() -> UiRendererRegistry {
    let mut registry = UiRendererRegistry::new();

    registry.register(super::templates::TableRenderer::new());
    registry.register(super::templates::ChartRenderer::new());
    registry.register(super::templates::TextRenderer::new());
    registry.register(super::templates::FormRenderer::new());
    registry.register(super::templates::ListRenderer::new());
    registry.register(super::templates::ImageRenderer::new());
    registry.register(super::templates::DashboardRenderer::new());

    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = create_default_registry();
        assert!(registry.supports("table"));
        assert!(registry.supports("chart"));
        assert!(registry.supports("text"));
    }

    #[test]
    fn test_unsupported_type() {
        let registry = UiRendererRegistry::new();
        assert!(!registry.supports("unknown_type"));
        assert!(registry.get("unknown_type").is_none());
    }
}
