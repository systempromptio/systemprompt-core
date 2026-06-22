//! Tests for `ExtensionRegistry::discover` / `discover_and_merge` in
//! `registry/discovery.rs`.
//!
//! The unit-test binary registers no extensions through `inventory`, so
//! `discover()` exercises the empty-discovery (`warn!`) branch. `discover` also
//! consults the process-global injected list; these tests assert only
//! invariants that hold regardless of that global state, and use uniquely named
//! extensions for the merge assertions.

use std::sync::Arc;

use systemprompt_extension::{Extension, ExtensionMetadata, ExtensionRegistry};

struct NamedExt {
    id: &'static str,
}

impl Extension for NamedExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "Named",
            version: "1.0.0",
        }
    }
}

#[test]
fn discover_returns_ok() {
    let registry = ExtensionRegistry::discover().expect("discover should not error");
    // No inventory registrations in this binary, so discovery succeeds and the
    // registry is well-formed (validation over zero/declared extensions holds).
    registry.validate().expect("discovered registry validates");
}

#[test]
fn discover_and_merge_includes_injected_extension() {
    let injected: Vec<Arc<dyn Extension>> = vec![Arc::new(NamedExt {
        id: "merge-unit-only",
    })];

    let registry =
        ExtensionRegistry::discover_and_merge(injected).expect("discover_and_merge should succeed");

    assert!(
        registry.has("merge-unit-only"),
        "merged extension must be present in the registry"
    );
    let ext = registry.get("merge-unit-only").expect("present");
    assert_eq!(ext.id(), "merge-unit-only");
}

#[test]
fn discover_and_merge_rejects_duplicate_merge() {
    let registry = ExtensionRegistry::discover_and_merge(vec![
        Arc::new(NamedExt { id: "dup-merge" }) as Arc<dyn Extension>,
    ])
    .expect("first merge succeeds");
    assert!(registry.has("dup-merge"));

    // A second discover_and_merge call builds an independent registry, so the
    // same id merges cleanly again — duplicates are only rejected within one
    // registry, which `merge` of two identical ids exercises.
    let mut reg2 = ExtensionRegistry::new();
    reg2.register(Arc::new(NamedExt { id: "dup-merge" }))
        .expect("first register");
    let err = reg2
        .merge(vec![Arc::new(NamedExt { id: "dup-merge" })])
        .expect_err("duplicate id within one registry must fail");
    assert!(err.to_string().contains("dup-merge"));
}
