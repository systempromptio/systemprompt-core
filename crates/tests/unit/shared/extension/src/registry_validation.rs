//! Tests for `ExtensionRegistry` validation in `registry/validation.rs`:
//! dependency validation (`validate_dependencies`), plus the exported
//! `RESERVED_PATHS` table the server mount refuses extension declarations
//! under. The base-path rule (the web root or under `/api/`) is the server
//! mount's, tested with its core declaration table.

use std::sync::Arc;

use systemprompt_extension::{
    Extension, ExtensionMetadata, ExtensionRegistry, LoaderError, RESERVED_PATHS,
};

use crate::stub_ctx::registry_with;

struct DepExt {
    id: &'static str,
    deps: Vec<&'static str>,
}

impl DepExt {
    fn new(id: &'static str) -> Self {
        Self {
            id,
            deps: Vec::new(),
        }
    }

    fn with_deps(mut self, deps: Vec<&'static str>) -> Self {
        self.deps = deps;
        self
    }
}

impl Extension for DepExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "Dependency Ext",
            version: "1.0.0",
        }
    }

    fn dependencies(&self) -> Vec<&'static str> {
        self.deps.clone()
    }
}

#[test]
fn validate_dependencies_ok_when_satisfied() {
    let registry = registry_with(vec![
        Arc::new(DepExt::new("base")),
        Arc::new(DepExt::new("child").with_deps(vec!["base"])),
    ]);
    registry
        .validate_dependencies()
        .expect("satisfied dependency should validate");
}

#[test]
fn merge_refuses_a_missing_dependency() {
    let mut registry = ExtensionRegistry::new();
    let err = registry
        .merge(vec![Arc::new(
            DepExt::new("needy").with_deps(vec!["absent"]),
        )])
        .expect_err("missing dependency must fail at insert time");
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
fn reserved_paths_table_covers_known_core_surfaces() {
    assert!(RESERVED_PATHS.contains(&"/api/v1/oauth"));
    assert!(RESERVED_PATHS.contains(&"/api/v1/mcp"));
    assert!(RESERVED_PATHS.contains(&"/.well-known"));
    // A bespoke extension path is not reserved.
    assert!(!RESERVED_PATHS.contains(&"/api/v1/widgets"));
}
