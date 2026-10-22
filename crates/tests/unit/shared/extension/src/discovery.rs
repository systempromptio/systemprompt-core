//! Tests for `ExtensionRegistry::discover` / `discover_with` in
//! `registry/discovery.rs`.
//!
//! The unit-test binary registers exactly one extension through `inventory`
//! (`InventoryExt`, below), so `discover()` exercises the inventory-iteration
//! branch; `discover_with` merges the `InjectedExtensions` value it is handed.
//! These tests use uniquely named extensions for the merge assertions and
//! account for the single inventory registration in count assertions.


use systemprompt_extension::{Extension, ExtensionMetadata, ExtensionRegistry};

const INVENTORY_ID: &str = "inventory-registered-unit-ext";

fn debug_subscriber_guard() -> tracing::subscriber::DefaultGuard {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_test_writer()
        .finish();
    tracing::subscriber::set_default(subscriber)
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
    registry.validate().expect("discovered registry validates");
    assert_eq!(
        registry.len(),
        1,
        "plain discovery sees the inventory registration and nothing injected"
    );
}

#[test]
fn discover_with_includes_injected_extensions_and_skips_duplicate_ids() {
    // Injecting the same id twice must collapse to a single registry entry (the
    // second is skipped as already-discovered), while a distinct id is also
    // included. A DEBUG subscriber is active so the injected-extension log
    // fields (count, name, priority) and the completion-summary fields are
    // evaluated.
    let _guard = debug_subscriber_guard();

    let registry = ExtensionRegistry::discover_with(&crate::injected_lock::injected())
        .expect("discover should not error");

    assert!(
        registry.has(crate::injected_lock::PRIMARY_ID),
        "injected id must be included"
    );
    assert!(
        registry.has(crate::injected_lock::SECONDARY_ID),
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
