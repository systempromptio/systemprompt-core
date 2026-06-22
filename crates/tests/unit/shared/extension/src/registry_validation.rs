//! Tests for `ExtensionRegistry` validation in `registry/validation.rs`:
//! dependency validation (`validate_dependencies`) and API base-path checks
//! (`validate_api_paths`), plus the exported `RESERVED_PATHS` table.

use std::sync::Arc;

use systemprompt_extension::{
    Extension, ExtensionContext, ExtensionMetadata, ExtensionRegistry, ExtensionRouter,
    LoaderError, RESERVED_PATHS,
};
use systemprompt_traits::{ConfigProvider, DatabaseHandle};

struct RouterExt {
    id: &'static str,
    base_path: Option<&'static str>,
    deps: Vec<&'static str>,
}

impl RouterExt {
    fn new(id: &'static str) -> Self {
        Self {
            id,
            base_path: None,
            deps: Vec::new(),
        }
    }

    fn with_base_path(mut self, path: &'static str) -> Self {
        self.base_path = Some(path);
        self
    }

    fn with_deps(mut self, deps: Vec<&'static str>) -> Self {
        self.deps = deps;
        self
    }
}

impl Extension for RouterExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "Router Ext",
            version: "1.0.0",
        }
    }

    fn router(&self, _ctx: &dyn ExtensionContext) -> Option<ExtensionRouter> {
        self.base_path
            .map(|p| ExtensionRouter::new(axum::Router::new(), p))
    }

    fn dependencies(&self) -> Vec<&'static str> {
        self.deps.clone()
    }
}

struct StubCtx;

#[derive(Debug)]
struct StubConfig;

impl ConfigProvider for StubConfig {
    fn get(&self, _key: &str) -> Option<String> {
        None
    }
    fn database_url(&self) -> &str {
        "postgres://x/y"
    }
    fn system_path(&self) -> &str {
        "/tmp"
    }
    fn api_port(&self) -> u16 {
        0
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug)]
struct StubDb;

impl DatabaseHandle for StubDb {
    fn is_connected(&self) -> bool {
        true
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ExtensionContext for StubCtx {
    fn config(&self) -> Arc<dyn ConfigProvider> {
        Arc::new(StubConfig)
    }
    fn database(&self) -> Arc<dyn DatabaseHandle> {
        Arc::new(StubDb)
    }
    fn get_extension(&self, _id: &str) -> Option<Arc<dyn Extension>> {
        None
    }
}

fn registry_with(exts: Vec<Arc<dyn Extension>>) -> ExtensionRegistry {
    let mut registry = ExtensionRegistry::new();
    registry.merge(exts).expect("merge stub extensions");
    registry
}

#[test]
fn validate_dependencies_ok_when_satisfied() {
    let registry = registry_with(vec![
        Arc::new(RouterExt::new("base")),
        Arc::new(RouterExt::new("child").with_deps(vec!["base"])),
    ]);
    registry
        .validate_dependencies()
        .expect("satisfied dependency should validate");
}

#[test]
fn validate_dependencies_reports_missing() {
    let registry = registry_with(vec![Arc::new(
        RouterExt::new("needy").with_deps(vec!["absent"]),
    )]);

    let err = registry
        .validate_dependencies()
        .expect_err("missing dependency must fail");
    match err {
        LoaderError::MissingDependency {
            extension,
            dependency,
        } => {
            assert_eq!(extension, "needy");
            assert_eq!(dependency, "absent");
        },
        other => panic!("expected MissingDependency, got {other:?}"),
    }
}

#[test]
fn validate_api_paths_accepts_valid_unreserved_path() {
    let registry = registry_with(vec![Arc::new(
        RouterExt::new("ok").with_base_path("/api/v1/widgets"),
    )]);
    registry
        .validate_api_paths(&StubCtx)
        .expect("a non-reserved /api/ path is valid");
}

#[test]
fn validate_api_paths_ignores_extensions_without_a_router() {
    let registry = registry_with(vec![Arc::new(RouterExt::new("no-router"))]);
    registry
        .validate_api_paths(&StubCtx)
        .expect("extensions without a router impose no path constraint");
}

#[test]
fn validate_api_paths_rejects_non_api_prefix() {
    let registry = registry_with(vec![Arc::new(
        RouterExt::new("bad").with_base_path("/widgets"),
    )]);

    let err = registry
        .validate_api_paths(&StubCtx)
        .expect_err("base path outside /api/ must fail");
    match err {
        LoaderError::InvalidBasePath { extension, path } => {
            assert_eq!(extension, "bad");
            assert_eq!(path, "/widgets");
        },
        other => panic!("expected InvalidBasePath, got {other:?}"),
    }
}

#[test]
fn validate_api_paths_rejects_reserved_path_collision() {
    let registry = registry_with(vec![Arc::new(
        RouterExt::new("colliding").with_base_path("/api/v1/oauth/extra"),
    )]);

    let err = registry
        .validate_api_paths(&StubCtx)
        .expect_err("a path under a reserved prefix must fail");
    match err {
        LoaderError::ReservedPathCollision { extension, path } => {
            assert_eq!(extension, "colliding");
            assert_eq!(path, "/api/v1/oauth/extra");
        },
        other => panic!("expected ReservedPathCollision, got {other:?}"),
    }
}

#[test]
fn reserved_paths_table_covers_known_core_surfaces() {
    assert!(RESERVED_PATHS.contains(&"/api/v1/oauth"));
    assert!(RESERVED_PATHS.contains(&"/api/v1/mcp"));
    assert!(RESERVED_PATHS.contains(&"/.well-known"));
    // A bespoke extension path is not reserved.
    assert!(!RESERVED_PATHS.contains(&"/api/v1/widgets"));
}
