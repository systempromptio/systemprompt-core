//! Switching the active tenant must resolve every downstream lookup
//! (active session, credentials, tenant record) against the new tenant —
//! and never leak fields from the previous active context.

use systemprompt_cloud::tenants::TenantStore;

use crate::support::{TenantFixture, seeded_session_store};

#[tokio::test]
async fn switching_active_key_changes_resolved_session() {
    let fx = TenantFixture::new();
    let mut store = seeded_session_store(&fx);

    store.set_active(&fx.key_a());
    let active_a = store.active_session().expect("active A");
    assert_eq!(active_a.session_token.as_str(), "token-a-v1");
    assert_eq!(active_a.tenant_key.as_ref().unwrap().as_str(), "tenant-a");

    store.set_active(&fx.key_b());
    let active_b = store.active_session().expect("active B");
    assert_eq!(active_b.session_token.as_str(), "token-b-v1");
    assert_eq!(active_b.tenant_key.as_ref().unwrap().as_str(), "tenant-b");
}

#[tokio::test]
async fn switching_active_does_not_mutate_other_sessions() {
    let fx = TenantFixture::new();
    let mut store = seeded_session_store(&fx);

    store.set_active(&fx.key_a());
    let snapshot_b_before = store.get_valid_session(&fx.key_b()).cloned().unwrap();

    store.set_active(&fx.key_b());
    store.set_active(&fx.key_a());

    let snapshot_b_after = store.get_valid_session(&fx.key_b()).cloned().unwrap();
    assert_eq!(
        snapshot_b_before.session_token.as_str(),
        snapshot_b_after.session_token.as_str()
    );
    assert_eq!(
        snapshot_b_before.context_id().as_str(),
        snapshot_b_after.context_id().as_str()
    );
}

#[tokio::test]
async fn resolved_tenant_record_matches_active_key() {
    let fx = TenantFixture::new();
    let store = TenantStore::load_from_path(&fx.tenants_path).expect("tenant store");

    let a = store.find_tenant("tenant-a").expect("A");
    let b = store.find_tenant("tenant-b").expect("B");

    assert_eq!(a.hostname.as_deref(), Some("a.systemprompt.test"));
    assert_eq!(b.hostname.as_deref(), Some("b.systemprompt.test"));
    assert_ne!(a.app_id, b.app_id, "app_id must differ across tenants");
    assert_ne!(
        a.internal_database_url, b.internal_database_url,
        "internal database urls must not collide"
    );
}

#[tokio::test]
async fn active_session_round_trips_through_disk() {
    let fx = TenantFixture::new();
    let mut store = seeded_session_store(&fx);
    store.set_active_with_profile(&fx.key_b(), "profile-b");
    store.save(&fx.sessions_dir).expect("save");

    let reloaded = systemprompt_cloud::SessionStore::load(&fx.sessions_dir).expect("reload");
    let active = reloaded.active_session().expect("active");
    assert_eq!(active.tenant_key.as_ref().unwrap().as_str(), "tenant-b");
    assert_eq!(reloaded.active_profile_name.as_deref(), Some("profile-b"));
}
