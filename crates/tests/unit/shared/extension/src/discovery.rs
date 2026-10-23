//! Tests for `ExtensionRegistry::discover` in `registry/discovery.rs`.
//!
//! The unit-test binary registers exactly one extension through `inventory`
//! (`InventoryExt`, below), the one way an extension exists, so `discover()`
//! exercises the inventory-iteration branch; count assertions account for
//! that single registration.


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
        "discovery sees exactly the inventory registration"
    );
}
