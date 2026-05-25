//! Credential and session-token rotation must invalidate the previous
//! value with no silent fallback to the cached copy.

use chrono::Duration;
use systemprompt_cloud::cli_session::{CliSession, SessionStore};
use systemprompt_cloud::CloudCredentials;

use crate::support::{build_session_for, jwt_token, save_credentials, TenantFixture};

#[tokio::test]
async fn rotating_session_token_invalidates_previous_handle() {
    let fx = TenantFixture::new();
    let mut store = SessionStore::new();
    let original = build_session_for(
        "profile-a",
        &fx.key_a(),
        "token-v1",
        "00000000-0000-4000-8000-000000000001",
    );
    store.upsert_session(&fx.key_a(), original.clone());
    store.save(&fx.sessions_dir).expect("save");

    // Rotate.
    let rotated = build_session_for(
        "profile-a",
        &fx.key_a(),
        "token-v2",
        "00000000-0000-4000-8000-000000000002",
    );
    store.upsert_session(&fx.key_a(), rotated);
    store.save(&fx.sessions_dir).expect("save");

    let reloaded = SessionStore::load(&fx.sessions_dir).expect("reload");
    let current = reloaded.get_valid_session(&fx.key_a()).expect("current");
    assert_eq!(current.session_token.as_str(), "token-v2");
    assert_ne!(
        current.session_token.as_str(),
        original.session_token.as_str(),
        "old token must not survive rotation"
    );
}

#[tokio::test]
async fn rotating_credentials_overwrites_previous_token_on_disk() {
    let fx = TenantFixture::new();

    let v1 = jwt_token(3600);
    save_credentials(&fx.credentials_path, &v1, "user@example.com");

    let v2 = jwt_token(7200);
    save_credentials(&fx.credentials_path, &v2, "user@example.com");

    let loaded = CloudCredentials::load_from_path(&fx.credentials_path).expect("load");
    assert_eq!(loaded.api_token, v2);
    assert_ne!(loaded.api_token, v1, "v1 must not remain after overwrite");
}

#[tokio::test]
async fn expired_session_is_filtered_by_valid_lookup() {
    let fx = TenantFixture::new();
    let mut store = SessionStore::new();

    let mut session = build_session_for(
        "profile-a",
        &fx.key_a(),
        "token-old",
        "00000000-0000-4000-8000-000000000003",
    );
    session.expires_at = chrono::Utc::now() - Duration::seconds(1);
    store.upsert_session(&fx.key_a(), session);

    assert!(
        store.get_valid_session(&fx.key_a()).is_none(),
        "expired session must not satisfy a valid lookup"
    );
    // The raw get is still present until prune is invoked.
    assert!(store.get_session(&fx.key_a()).is_some());

    let pruned = store.prune_expired();
    assert_eq!(pruned, 1);
    assert!(store.get_session(&fx.key_a()).is_none());
}

#[tokio::test]
async fn empty_token_session_is_treated_as_invalid() {
    let fx = TenantFixture::new();
    let mut store = SessionStore::new();
    // An empty session_token is the on-disk marker for "no live creds"; the
    // lookup must reject it even before the cred check runs server-side.
    let session = build_session_for(
        "profile-a",
        &fx.key_a(),
        "",
        "00000000-0000-4000-8000-000000000004",
    );
    store.upsert_session(&fx.key_a(), session);

    assert!(store.get_valid_session(&fx.key_a()).is_none());
}

#[tokio::test]
async fn cloned_session_handle_does_not_self_rotate() {
    // Defence-in-depth: a CliSession is Clone. Cloning then rotating the
    // store must not retro-update the clone — i.e. there is no shared
    // mutable state hiding behind the type.
    let fx = TenantFixture::new();
    let session = build_session_for(
        "profile-a",
        &fx.key_a(),
        "token-snapshot",
        "00000000-0000-4000-8000-000000000005",
    );
    let snapshot: CliSession = session.clone();

    let mut store = SessionStore::new();
    store.upsert_session(&fx.key_a(), session);
    let rotated = build_session_for(
        "profile-a",
        &fx.key_a(),
        "token-fresh",
        "00000000-0000-4000-8000-000000000006",
    );
    store.upsert_session(&fx.key_a(), rotated);

    assert_eq!(snapshot.session_token.as_str(), "token-snapshot");
}
