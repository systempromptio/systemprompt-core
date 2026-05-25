//! A session minted for tenant A must never satisfy a request for tenant B.

use systemprompt_cloud::cli_session::{SessionKey, SessionStore};
use systemprompt_identifiers::TenantId;

use crate::support::{
    build_session_a, build_session_b, build_session_for, seeded_session_store, TenantFixture,
};

#[tokio::test]
async fn tenant_a_session_does_not_satisfy_tenant_b_lookup() {
    let fx = TenantFixture::new();
    let store = seeded_session_store(&fx);

    let a = store.get_valid_session(&fx.key_a()).expect("A session");
    assert!(a.is_valid_for_tenant(&fx.key_a()));
    assert!(
        !a.is_valid_for_tenant(&fx.key_b()),
        "A's session must not validate against B"
    );
}

#[tokio::test]
async fn unknown_tenant_lookup_returns_none() {
    let fx = TenantFixture::new();
    let store = seeded_session_store(&fx);

    let missing = SessionKey::Tenant(TenantId::new("tenant-ghost"));
    assert!(
        store.get_valid_session(&missing).is_none(),
        "lookup for an unminted tenant must return None"
    );
}

#[tokio::test]
async fn local_session_does_not_satisfy_tenant_request() {
    let fx = TenantFixture::new();
    let mut store = SessionStore::new();
    let local = build_session_for(
        "local",
        &SessionKey::Local,
        "token-local",
        "00000000-0000-4000-8000-0000000000cc",
    );
    store.upsert_session(&SessionKey::Local, local);
    store.save(&fx.sessions_dir).expect("save");
    let store = SessionStore::load(&fx.sessions_dir).expect("reload");

    let got = store.get_valid_session(&fx.key_a());
    assert!(got.is_none(), "local-only session must not authorise tenant A");
}

#[tokio::test]
async fn session_a_token_does_not_appear_under_b_storage_key() {
    let fx = TenantFixture::new();
    let _ = seeded_session_store(&fx);

    // Re-read the raw index.json to assert on-disk isolation, not just
    // accessor behaviour. The serialised file must scope tokens by storage key.
    let raw = std::fs::read_to_string(fx.sessions_dir.join("index.json")).expect("read index");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("parse");

    let sessions = parsed
        .get("sessions")
        .and_then(|v| v.as_object())
        .expect("sessions object");
    let key_a = format!("tenant_{}", fx.tenant_a);
    let key_b = format!("tenant_{}", fx.tenant_b);

    let a_token = sessions[&key_a]["session_token"].as_str().expect("a token");
    let b_token = sessions[&key_b]["session_token"].as_str().expect("b token");

    assert_ne!(a_token, b_token, "tokens must differ across tenants");
    assert_eq!(a_token, "token-a-v1");
    assert_eq!(b_token, "token-b-v1");
}

#[tokio::test]
async fn upsert_under_b_does_not_overwrite_a() {
    let fx = TenantFixture::new();
    let mut store = SessionStore::new();
    store.upsert_session(&fx.key_a(), build_session_a(&fx));
    store.upsert_session(&fx.key_b(), build_session_b(&fx));

    let a_before = store.get_valid_session(&fx.key_a()).unwrap().session_token.clone();

    // Upsert a *new* B session and re-check A.
    let new_b = build_session_for(
        "profile-b2",
        &fx.key_b(),
        "token-b-rotated",
        "00000000-0000-4000-8000-0000000000b2",
    );
    store.upsert_session(&fx.key_b(), new_b);

    let a_after = store.get_valid_session(&fx.key_a()).unwrap().session_token.clone();
    assert_eq!(a_before.as_str(), a_after.as_str(), "A token must survive B rotation");
    let b_after = store.get_valid_session(&fx.key_b()).unwrap().session_token.clone();
    assert_eq!(b_after.as_str(), "token-b-rotated");
}
