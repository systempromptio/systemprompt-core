//! Shared scaffolding for cloud isolation tests.

use base64::prelude::*;
use chrono::Utc;
use std::path::PathBuf;
use systemprompt_cloud::CloudCredentials;
use systemprompt_cloud::cli_session::{
    CliSession, CliSessionBuilder, SessionIdentity, SessionKey, SessionStore,
};
use systemprompt_cloud::tenants::{NewCloudTenantParams, StoredTenant, TenantStore};
use systemprompt_identifiers::{ContextId, Email, ProfileName, SessionId, SessionToken, TenantId};
use systemprompt_models::auth::UserType;
use systemprompt_test_fixtures::fixture_user_id;
use tempfile::TempDir;

const CONTEXT_A: &str = "00000000-0000-4000-8000-00000000000a";
const CONTEXT_B: &str = "00000000-0000-4000-8000-00000000000b";

/// A two-tenant fixture rooted at a temp dir. Models the on-disk layout of
/// `~/.systemprompt/` so isolation tests never reach for the real one.
pub struct TenantFixture {
    pub _temp: TempDir,
    pub tenants_path: PathBuf,
    pub sessions_dir: PathBuf,
    pub credentials_path: PathBuf,
    pub tenant_a: TenantId,
    pub tenant_b: TenantId,
}

impl TenantFixture {
    pub fn new() -> Self {
        let temp = TempDir::new().expect("tempdir");
        let base = temp.path();
        let tenants_path = base.join("tenants.json");
        let sessions_dir = base.join("sessions");
        let credentials_path = base.join("credentials.json");

        let tenant_a = TenantId::new("tenant-a");
        let tenant_b = TenantId::new("tenant-b");

        let store = TenantStore::new(vec![
            StoredTenant::new_cloud(NewCloudTenantParams {
                id: tenant_a.as_str().to_string(),
                name: "Tenant A".to_string(),
                app_id: Some("app-a".to_string()),
                hostname: Some("a.systemprompt.test".to_string()),
                region: Some("iad".to_string()),
                database_url: Some("postgres://a.example/a".to_string()),
                internal_database_url: "postgres://internal-a/a".to_string(),
                external_db_access: false,
            }),
            StoredTenant::new_cloud(NewCloudTenantParams {
                id: tenant_b.as_str().to_string(),
                name: "Tenant B".to_string(),
                app_id: Some("app-b".to_string()),
                hostname: Some("b.systemprompt.test".to_string()),
                region: Some("lhr".to_string()),
                database_url: Some("postgres://b.example/b".to_string()),
                internal_database_url: "postgres://internal-b/b".to_string(),
                external_db_access: false,
            }),
        ]);
        store
            .save_to_path(&tenants_path)
            .expect("save tenant store");

        Self {
            _temp: temp,
            tenants_path,
            sessions_dir,
            credentials_path,
            tenant_a,
            tenant_b,
        }
    }

    pub fn key_a(&self) -> SessionKey {
        SessionKey::Tenant(self.tenant_a.clone())
    }

    pub fn key_b(&self) -> SessionKey {
        SessionKey::Tenant(self.tenant_b.clone())
    }
}

pub fn build_session_for(
    profile: &str,
    key: &SessionKey,
    token: &str,
    context: &str,
) -> CliSession {
    CliSessionBuilder::new(
        ProfileName::new(profile),
        SessionToken::new(token),
        SessionId::new(format!("sid-{profile}")),
        ContextId::new(context),
        SessionIdentity::new(
            fixture_user_id(),
            Email::new(format!("{profile}@example.com")),
            UserType::User,
        ),
    )
    .with_session_key(key)
    .build()
}

pub fn build_session_a(fx: &TenantFixture) -> CliSession {
    build_session_for("profile-a", &fx.key_a(), "token-a-v1", CONTEXT_A)
}

pub fn build_session_b(fx: &TenantFixture) -> CliSession {
    build_session_for("profile-b", &fx.key_b(), "token-b-v1", CONTEXT_B)
}

/// Build a valid (non-expired) JWT-shaped token for credential tests.
pub fn jwt_token(exp_offset_secs: i64) -> String {
    let header = BASE64_URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
    let exp = Utc::now().timestamp() + exp_offset_secs;
    let payload = BASE64_URL_SAFE_NO_PAD.encode(format!(r#"{{"exp":{}}}"#, exp));
    let signature = BASE64_URL_SAFE_NO_PAD.encode("sig");
    format!("{header}.{payload}.{signature}")
}

pub fn save_credentials(path: &std::path::Path, token: &str, email: &str) {
    let creds = CloudCredentials::new(
        token.to_string(),
        "https://api.systemprompt.test".to_string(),
        email.to_string(),
    );
    creds.save_to_path(path).expect("save credentials");
}

/// Seed both tenants with sessions, persist to disk, and return a reloaded
/// `SessionStore`. This is the integration-shaped path: every test re-reads
/// from disk to detect cross-test in-memory leakage.
pub fn seeded_session_store(fx: &TenantFixture) -> SessionStore {
    let mut store = SessionStore::new();
    store.upsert_session(&fx.key_a(), build_session_a(fx));
    store.upsert_session(&fx.key_b(), build_session_b(fx));
    store.save(&fx.sessions_dir).expect("save session store");

    SessionStore::load(&fx.sessions_dir).expect("reload session store")
}
