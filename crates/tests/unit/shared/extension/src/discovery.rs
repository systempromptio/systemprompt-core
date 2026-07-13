//! Tests for `ExtensionRegistry::discover` / `discover_and_merge` in
//! `registry/discovery.rs`.
//!
//! The unit-test binary registers exactly one extension through `inventory`
//! (`InventoryExt`, below), so `discover()` exercises the inventory-iteration
//! branch as well as the injected-extension path. `discover` also consults the
//! process-global injected list; these tests use uniquely named extensions for
//! the merge assertions and account for the single inventory registration in
//! count assertions.

use std::sync::Arc;

use systemprompt_extension::runtime_config::{
    InjectedExtensions, WebAssetsStrategy, set_injected_extensions,
};
use systemprompt_extension::{Extension, ExtensionMetadata, ExtensionRegistry};

const INVENTORY_ID: &str = "inventory-registered-unit-ext";

fn debug_subscriber_guard() -> tracing::subscriber::DefaultGuard {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_test_writer()
        .finish();
    tracing::subscriber::set_default(subscriber)
}

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

#[derive(Default)]
struct InventoryExt;

impl Extension for InventoryExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: INVENTORY_ID,
            name: "InventoryUnit",
            version: "1.0.0",
        }
    }
}

systemprompt_extension::register_extension!(InventoryExt);

#[test]
fn discover_finds_the_inventory_registered_extension() {
    // Compile-time inventory registration must surface through `discover()`.
    // A DEBUG subscriber is active so the per-extension discovery log fields are
    // evaluated (the inventory-iteration branch).
    let _guard = debug_subscriber_guard();
    let registry = ExtensionRegistry::discover().expect("discover should not error");
    assert!(
        registry.has(INVENTORY_ID),
        "inventory-registered extension must be discovered"
    );
    let ext = registry.get(INVENTORY_ID).expect("present");
    assert_eq!(ext.id(), INVENTORY_ID);
}

#[test]
fn discover_returns_ok() {
    let registry = ExtensionRegistry::discover().expect("discover should not error");
    // No inventory registrations in this binary, so discovery succeeds and the
    // registry is well-formed (validation over zero/declared extensions holds).
    registry.validate().expect("discovered registry validates");
}

#[test]
fn discover_includes_process_injected_extensions_and_skips_duplicate_ids() {
    // The process-global injected list is consulted by `discover()`. Injecting
    // the same id twice must collapse to a single registry entry (the second is
    // skipped as already-discovered), while a distinct id is also included.
    // Under cargo-nextest each test runs in its own process, so this one-shot
    // `set` cannot collide with the other tests in this binary. A DEBUG
    // subscriber is active so the injected-extension log fields (count, name,
    // priority) and the completion-summary fields are evaluated.
    let _guard = debug_subscriber_guard();
    let injected: Vec<Arc<dyn Extension>> = vec![
        Arc::new(NamedExt { id: "inj-primary" }),
        Arc::new(NamedExt { id: "inj-primary" }),
        Arc::new(NamedExt {
            id: "inj-secondary",
        }),
    ];
    set_injected_extensions(InjectedExtensions {
        extensions: injected,
        web_assets: WebAssetsStrategy::Disabled,
    })
    .expect("injected extensions may be set exactly once per process");

    let registry = ExtensionRegistry::discover().expect("discover should not error");

    assert!(registry.has("inj-primary"), "injected id must be included");
    assert!(
        registry.has("inj-secondary"),
        "second distinct injected id must be included"
    );
    assert!(!registry.is_empty());
    assert_eq!(
        registry.len(),
        3,
        "one inventory extension plus two distinct injected ids; the duplicate \
         injected id must be skipped, not double-counted"
    );
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
