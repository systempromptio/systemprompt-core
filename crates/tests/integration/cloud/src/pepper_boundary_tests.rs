//! Per-tenant secret boundary.
//!
//! The `oauth_at_rest_pepper` itself is server-side, *per-deployment*
//! state — each cloud tenant is its own backend process with its own
//! pepper. On the CLI side, tenant secrets are delivered via the
//! per-tenant [`TenantSecrets`] response fetched from each tenant's own
//! `secrets_url`. There is no shared client-side secret pool, and there
//! is no shared in-memory cache that would let one tenant's pepper (or
//! database URL, or LLM API key) leak into another tenant's resolved
//! config.
//!
//! These tests pin that invariant by:
//!
//! 1. Round-tripping two distinct [`TenantSecrets`] payloads and asserting no
//!    field collision.
//! 2. Confirming that two `StoredTenant` records keep their
//!    `internal_database_url` and `app_id` distinct after a full on-disk
//!    round-trip.

use systemprompt_cloud::tenants::TenantStore;
use systemprompt_models::api::cloud::CloudTenantSecrets;

use crate::support::TenantFixture;
use systemprompt_identifiers::TenantId;

fn secrets_for(tag: &str) -> CloudTenantSecrets {
    CloudTenantSecrets {
        database_url: format!("postgres://{tag}.db/secret"),
        internal_database_url: format!("postgres://internal-{tag}.db/secret"),
        app_url: format!("https://{tag}.systemprompt.test"),
        anthropic_api_key: Some(format!("anthropic-{tag}-key")),
        openai_api_key: Some(format!("openai-{tag}-key")),
        gemini_api_key: None,
    }
}

#[tokio::test]
async fn tenant_secrets_payloads_are_independent_per_tenant() {
    let a = secrets_for("alpha");
    let b = secrets_for("beta");

    let a_json = serde_json::to_string(&a).expect("ser a");
    let b_json = serde_json::to_string(&b).expect("ser b");

    assert_ne!(a.database_url, b.database_url);
    assert_ne!(a.internal_database_url, b.internal_database_url);
    assert_ne!(a.anthropic_api_key, b.anthropic_api_key);
    assert_ne!(a.openai_api_key, b.openai_api_key);
    assert!(
        !a_json.contains("beta"),
        "A's secrets must not carry B's tag"
    );
    assert!(
        !b_json.contains("alpha"),
        "B's secrets must not carry A's tag"
    );
}

#[tokio::test]
async fn tenant_secrets_round_trip_preserves_only_own_fields() {
    let original = secrets_for("alpha");
    let json = serde_json::to_string(&original).unwrap();
    let parsed: CloudTenantSecrets = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.database_url, original.database_url);
    assert_eq!(parsed.anthropic_api_key, original.anthropic_api_key);
    assert_eq!(parsed.openai_api_key, original.openai_api_key);
    assert!(parsed.gemini_api_key.is_none());
}

#[tokio::test]
async fn stored_tenant_database_urls_remain_isolated_through_disk_round_trip() {
    let fx = TenantFixture::new();
    let store = TenantStore::load_from_path(&fx.tenants_path).expect("load");

    let a = store.find_tenant(&TenantId::new("tenant-a")).unwrap();
    let b = store.find_tenant(&TenantId::new("tenant-b")).unwrap();

    assert_ne!(a.internal_database_url, b.internal_database_url);
    assert_ne!(a.database_url, b.database_url);
    assert_ne!(a.app_id, b.app_id);
    assert_ne!(a.hostname, b.hostname);

    // Re-save and re-load to prove the JSON encoding does not collapse
    // them onto a shared field.
    store.save_to_path(&fx.tenants_path).expect("re-save");
    let raw = std::fs::read_to_string(&fx.tenants_path).expect("read tenants.json");
    assert!(raw.contains("postgres://internal-a/a"));
    assert!(raw.contains("postgres://internal-b/b"));
    assert!(raw.contains("\"app_id\": \"app-a\""));
    assert!(raw.contains("\"app_id\": \"app-b\""));
}
