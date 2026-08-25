use chrono::Duration;
use systemprompt_cloud::SessionBinding;
use systemprompt_cloud::cli_session::{
    CliSession, CliSessionBuilder, SessionIdentity, SessionKey, SessionStore,
};
use systemprompt_identifiers::{ContextId, Email, ProfileName, SessionId, SessionToken};
use systemprompt_models::auth::UserType;
use systemprompt_test_fixtures::fixture_user_id;
use tempfile::TempDir;

const ISSUER: &str = "http://localhost:8080";
const ROTATED_ISSUER: &str = "http://localhost:8081";
const CONTEXT_ID: &str = "00000000-0000-4000-8000-000000000001";

fn session_minted_under(issuer: &str) -> CliSession {
    CliSessionBuilder::new(
        SessionBinding::new(ProfileName::new("local"), issuer.to_owned()),
        SessionToken::new("token-abc"),
        SessionId::new("sid-001"),
        ContextId::new_unchecked(CONTEXT_ID),
        SessionIdentity::new(
            fixture_user_id(),
            Email::new("test@example.com"),
            UserType::Admin,
        ),
    )
    .build()
}

fn store_holding(issuer: &str) -> SessionStore {
    let mut store = SessionStore::new();
    store.upsert_session(&SessionKey::Local, session_minted_under(issuer));
    store
}

#[test]
fn a_session_is_reused_when_the_issuer_still_matches() {
    let store = store_holding(ISSUER);

    assert!(
        store
            .get_valid_session(&SessionKey::Local, ISSUER)
            .is_some()
    );
}

#[test]
fn a_session_minted_under_a_previous_issuer_is_not_reused() {
    let store = store_holding(ISSUER);

    assert!(
        store
            .get_valid_session(&SessionKey::Local, ROTATED_ISSUER)
            .is_none(),
        "the token would be rejected by every server validating the new issuer"
    );
}

#[test]
fn the_mutable_lookup_applies_the_same_issuer_check() {
    let mut store = store_holding(ISSUER);

    assert!(
        store
            .get_valid_session_mut(&SessionKey::Local, ROTATED_ISSUER)
            .is_none()
    );
    assert!(
        store
            .get_valid_session_mut(&SessionKey::Local, ISSUER)
            .is_some()
    );
}

#[test]
fn an_unexpired_stale_issuer_session_is_still_rejected() {
    let mut session = session_minted_under(ISSUER);
    session.expires_at = chrono::Utc::now() + Duration::hours(24);
    let mut store = SessionStore::new();
    store.upsert_session(&SessionKey::Local, session);

    assert!(
        store
            .get_valid_session(&SessionKey::Local, ROTATED_ISSUER)
            .is_none(),
        "expiry alone cannot detect an issuer change"
    );
}

#[test]
fn the_issuer_survives_a_save_and_reload() {
    let dir = TempDir::new().expect("temp dir");
    store_holding(ISSUER).save(dir.path()).expect("save store");

    let reloaded = SessionStore::load(dir.path())
        .unwrap()
        .expect("reload store");

    assert!(
        reloaded
            .get_valid_session(&SessionKey::Local, ISSUER)
            .is_some()
    );
    assert!(
        reloaded
            .get_valid_session(&SessionKey::Local, ROTATED_ISSUER)
            .is_none()
    );
}

#[test]
fn a_version_5_entry_is_rejected_rather_than_migrated() {
    let dir = TempDir::new().expect("temp dir");
    let index = dir.path().join("index.json");
    std::fs::write(
        &index,
        serde_json::json!({
            "version": 1,
            "sessions": {
                "local": {
                    "version": 5,
                    "profile_name": "local",
                    "session_token": "token-abc",
                    "session_id": "sid-001",
                    "context_id": CONTEXT_ID,
                    "user_id": fixture_user_id().as_str(),
                    "user_email": "test@example.com",
                    "user_type": "admin",
                    "created_at": "2026-08-06T00:00:00Z",
                    "expires_at": "2099-08-06T00:00:00Z",
                    "last_used": "2026-08-06T00:00:00Z"
                }
            },
            "active_key": "local",
            "updated_at": "2026-08-06T00:00:00Z"
        })
        .to_string(),
    )
    .expect("seed a pre-issuer store");

    let err = SessionStore::load(dir.path())
        .expect_err("an entry with no recorded issuer cannot be checked and must not be adopted");
    assert!(
        err.to_string().contains("admin session switch"),
        "the rejection must name the recovery command, got: {err}"
    );
}

#[test]
fn profile_discovery_ignores_the_issuer() {
    let store = store_holding(ISSUER);

    assert!(
        store.active_session_for_profile_discovery().is_none(),
        "no active key was set"
    );

    let mut store = store_holding(ISSUER);
    store.set_active(&SessionKey::Local);

    assert!(
        store.active_session_for_profile_discovery().is_some(),
        "resolving which profile to load happens before the issuer is known"
    );
}

#[test]
fn a_builder_ttl_pins_the_entry_to_the_tokens_own_lifetime() {
    let session = CliSessionBuilder::new(
        SessionBinding::new(ProfileName::new("local"), ISSUER.to_owned()),
        SessionToken::new("token-abc"),
        SessionId::new("sid-001"),
        ContextId::new_unchecked(CONTEXT_ID),
        SessionIdentity::new(
            fixture_user_id(),
            Email::new("test@example.com"),
            UserType::Admin,
        ),
    )
    .with_ttl(Duration::hours(1))
    .build();

    let lifetime = session.expires_at - session.created_at;

    assert_eq!(lifetime.num_hours(), 1);
}
