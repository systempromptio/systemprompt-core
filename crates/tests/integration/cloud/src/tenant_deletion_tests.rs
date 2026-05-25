//! Removing a tenant must clear its session and leave no orphans —
//! neither in the `SessionStore` index nor in the persisted JSON.

use systemprompt_cloud::cli_session::SessionStore;
use systemprompt_cloud::tenants::TenantStore;
use systemprompt_cloud::StoredTenant;

use crate::support::{seeded_session_store, TenantFixture};

#[tokio::test]
async fn removing_session_for_b_does_not_touch_a() {
    let fx = TenantFixture::new();
    let mut store = seeded_session_store(&fx);

    let removed = store.remove_session(&fx.key_b()).expect("removed");
    assert_eq!(removed.session_token.as_str(), "token-b-v1");

    assert!(store.get_session(&fx.key_b()).is_none());
    assert!(store.get_valid_session(&fx.key_a()).is_some());
}

#[tokio::test]
async fn removing_active_tenant_clears_active_session_lookup() {
    let fx = TenantFixture::new();
    let mut store = seeded_session_store(&fx);
    store.set_active(&fx.key_b());

    store.remove_session(&fx.key_b());

    // active_key still holds the storage key but no live session resolves.
    assert!(
        store.active_session().is_none(),
        "active_session() must not resurrect a removed tenant"
    );
}

#[tokio::test]
async fn tenant_deletion_persists_through_disk_round_trip() {
    let fx = TenantFixture::new();
    let mut store = seeded_session_store(&fx);
    store.remove_session(&fx.key_a());
    store.save(&fx.sessions_dir).expect("save");

    let reloaded = SessionStore::load(&fx.sessions_dir).expect("reload");
    assert!(reloaded.get_session(&fx.key_a()).is_none());
    assert!(reloaded.get_session(&fx.key_b()).is_some());

    let raw = std::fs::read_to_string(fx.sessions_dir.join("index.json")).expect("read");
    assert!(!raw.contains("token-a-v1"), "removed token must not linger on disk");
    assert!(raw.contains("token-b-v1"), "remaining token must persist");
}

#[tokio::test]
async fn removing_tenant_record_from_tenant_store() {
    let fx = TenantFixture::new();
    let mut store = TenantStore::load_from_path(&fx.tenants_path).expect("load");
    assert_eq!(store.len(), 2);

    let new_tenants: Vec<StoredTenant> =
        store.tenants.drain(..).filter(|t| t.id != "tenant-a").collect();
    let new_store = TenantStore::new(new_tenants);
    new_store.save_to_path(&fx.tenants_path).expect("save");

    let reloaded = TenantStore::load_from_path(&fx.tenants_path).expect("reload");
    assert_eq!(reloaded.len(), 1);
    assert!(reloaded.find_tenant("tenant-a").is_none());
    assert!(reloaded.find_tenant("tenant-b").is_some());
}
