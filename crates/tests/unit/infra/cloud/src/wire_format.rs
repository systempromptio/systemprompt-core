//! Wire-format proof that the typed-identifier migration is byte-identical to
//! the old raw-String persisted schema for `StoredTenant` and
//! `CloudCredentials`.

use chrono::{TimeZone, Utc};
use systemprompt_cloud::CloudCredentials;
use systemprompt_cloud::tenants::{NewCloudTenantParams, StoredTenant};
use systemprompt_identifiers::{CloudAuthToken, Email, TenantId};

#[test]
fn stored_tenant_json_matches_legacy_string_schema() {
    let tenant = StoredTenant::new_cloud(NewCloudTenantParams {
        id: TenantId::new("tenant-42"),
        name: "Acme".to_string(),
        app_id: Some("app-7".to_string()),
        hostname: Some("acme.fly.dev".to_string()),
        region: Some("lhr".to_string()),
        database_url: Some("postgres://ext/db".to_string()),
        internal_database_url: "postgres://int/db".to_string(),
        external_db_access: true,
    });

    let json = serde_json::to_string(&tenant).unwrap();
    let expected = concat!(
        r#"{"id":"tenant-42","name":"Acme","app_id":"app-7","#,
        r#""hostname":"acme.fly.dev","region":"lhr","#,
        r#""database_url":"postgres://ext/db","#,
        r#""internal_database_url":"postgres://int/db","#,
        r#""tenant_type":"cloud","external_db_access":true}"#
    );
    assert_eq!(json, expected);

    let roundtripped: StoredTenant = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtripped.id, tenant.id);
    assert_eq!(roundtripped.name, tenant.name);
    assert_eq!(roundtripped.app_id, tenant.app_id);
    assert_eq!(roundtripped.hostname, tenant.hostname);
    assert_eq!(roundtripped.region, tenant.region);
    assert_eq!(roundtripped.database_url, tenant.database_url);
    assert_eq!(
        roundtripped.internal_database_url,
        tenant.internal_database_url
    );
    assert_eq!(roundtripped.tenant_type, tenant.tenant_type);
    assert_eq!(roundtripped.external_db_access, tenant.external_db_access);
    assert_eq!(roundtripped.shared_container_db, tenant.shared_container_db);
}

#[test]
fn cloud_credentials_json_matches_legacy_string_schema() {
    let at = Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap();
    let creds = CloudCredentials {
        api_token: CloudAuthToken::new("tok_abc123"),
        api_url: "https://api.systemprompt.io".to_string(),
        authenticated_at: at,
        user_email: Email::new("ops@example.com"),
        last_validated_at: Some(at),
    };

    let json = serde_json::to_string(&creds).unwrap();
    let expected = concat!(
        r#"{"api_token":"tok_abc123","#,
        r#""api_url":"https://api.systemprompt.io","#,
        r#""authenticated_at":"2026-01-02T03:04:05Z","#,
        r#""user_email":"ops@example.com","#,
        r#""last_validated_at":"2026-01-02T03:04:05Z"}"#
    );
    assert_eq!(json, expected);

    let roundtripped: CloudCredentials = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtripped.api_token, creds.api_token);
    assert_eq!(roundtripped.api_url, creds.api_url);
    assert_eq!(roundtripped.authenticated_at, creds.authenticated_at);
    assert_eq!(roundtripped.user_email, creds.user_email);
    assert_eq!(roundtripped.last_validated_at, creds.last_validated_at);
}
