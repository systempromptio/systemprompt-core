//! Registration, discovery and the engine halves.

use std::sync::Arc;

use sp_extension::{
    BackgroundService, CancellationToken, EngineExtension, ExtensionError, ExtensionMetadata,
    SpContext, SpExtension, SpExtensions, extension_id, register_sp_extension,
};

use crate::support::{context, id, plain};

#[derive(Default)]
struct Registered;

impl SpExtension for Registered {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata::new(extension_id!("registered"), "registered")
    }
}

register_sp_extension!(Registered);

#[test]
fn a_registered_extension_is_discovered() {
    let found = SpExtensions::discover().unwrap();
    assert!(
        found
            .entries()
            .iter()
            .any(|entry| entry.metadata.id == id("registered"))
    );
}

#[test]
fn a_higher_priority_shadows_a_stock_extension() {
    let resolved = SpExtensions::from_extensions([
        plain("drive", 100),
        plain("drive", 500),
        plain("brain", 100),
    ])
    .unwrap();
    let kept: Vec<_> = resolved
        .entries()
        .iter()
        .map(|entry| (entry.metadata.id.as_str(), entry.metadata.priority))
        .collect();
    assert_eq!(kept, [("drive", 500), ("brain", 100)]);
}

#[test]
fn a_missing_dependency_fails_discovery() {
    let dependent: Arc<dyn SpExtension> = Arc::new(crate::support::Plain(
        ExtensionMetadata::new(id("slack"), "slack").depends_on(id("notifications")),
    ));
    let error = SpExtensions::from_extensions([dependent]).unwrap_err();
    assert!(matches!(error, ExtensionError::Resolve(_)));
}

struct Half(ExtensionMetadata);

impl EngineExtension for Half {
    fn metadata(&self) -> ExtensionMetadata {
        self.0.clone()
    }
}

struct WithHalf {
    own: &'static str,
    half: &'static str,
}

struct Ticker;

#[async_trait::async_trait]
impl BackgroundService for Ticker {
    fn name(&self) -> &'static str {
        "ticker"
    }

    async fn run(&self, cancel: CancellationToken) {
        cancel.cancelled().await;
    }
}

impl SpExtension for WithHalf {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata::new(id(self.own), "with half")
    }

    fn provide(&self, ctx: &SpContext) {
        ctx.handles.insert(Arc::new(self.own.len()));
    }

    fn engine(&self, _ctx: &SpContext) -> Option<Arc<dyn EngineExtension>> {
        Some(Arc::new(Half(ExtensionMetadata::new(
            id(self.half),
            "half",
        ))))
    }

    fn services(&self, _ctx: &SpContext) -> Vec<Arc<dyn BackgroundService>> {
        vec![Arc::new(Ticker)]
    }
}

fn with_half(own: &'static str, half: &'static str) -> Arc<dyn SpExtension> {
    Arc::new(WithHalf { own, half })
}

#[tokio::test]
async fn engine_halves_resolve_under_their_extensions_id() {
    let root = tempfile::tempdir().unwrap();
    let ctx = context(root.path());
    let extensions = SpExtensions::from_extensions([with_half("drive", "drive")]).unwrap();
    extensions.provide(&ctx);
    assert_eq!(ctx.handles.get::<usize>().as_deref(), Some(&5));
    let set = extensions.engine_set(&ctx).unwrap();
    assert_eq!(set.entries().len(), 1);
    assert_eq!(set.entries()[0].metadata.id, id("drive"));
    let services = extensions.services(&ctx);
    assert_eq!(services.len(), 1);
    assert_eq!(services[0].extension, id("drive"));
    assert_eq!(services[0].item.name(), "ticker");
}

#[tokio::test]
async fn an_engine_half_under_another_id_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let ctx = context(root.path());
    let extensions = SpExtensions::from_extensions([with_half("drive", "brain")]).unwrap();
    let error = extensions.engine_set(&ctx).unwrap_err();
    assert!(matches!(
        error,
        ExtensionError::MismatchedHalf { extension, engine } if extension == id("drive") && engine == id("brain")
    ));
}
