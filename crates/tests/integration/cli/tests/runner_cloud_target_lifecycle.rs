//! Cloud execution routing through the public target resolver.

use std::fs;

use chrono::Duration;
use systemprompt_cli::runner::routing::{ExecutionTarget, determine_execution_target};
use systemprompt_cli_integration_tests::full_bootstrap::isolated_fixture;
use systemprompt_cloud::{
    CliSession, SessionBinding, SessionIdentity, SessionKey, SessionStore, StoredTenant,
    TenantStore,
};
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::{
    ContextId, Email, ProfileName, SessionId, SessionToken, TenantId, UserId,
};
use systemprompt_models::Profile;
use systemprompt_models::auth::UserType;
use systemprompt_models::profile::{CloudConfig, CloudValidationMode, ProfileType};

const ISSUER: &str = "https://routing-issuer.test";

struct WorkingDirectory(std::path::PathBuf);

impl WorkingDirectory {
    fn enter(path: &std::path::Path) -> Self {
        let previous = std::env::current_dir().expect("read original working directory");
        std::env::set_current_dir(path).expect("enter owned routing project");
        Self(previous)
    }
}

impl Drop for WorkingDirectory {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.0).expect("restore original working directory");
    }
}

fn session_for(key: &SessionKey, issuer: &str, token: &str, context: ContextId) -> CliSession {
    CliSession::builder(
        SessionBinding::new(
            ProfileName::try_new("subprocess_full").expect("valid profile name"),
            issuer.to_owned(),
        ),
        SessionToken::new(token),
        SessionId::new(format!("session_{}", uuid::Uuid::new_v4().simple())),
        context,
        SessionIdentity::new(
            UserId::new(format!("user_{}", uuid::Uuid::new_v4().simple())),
            Email::try_new("router@routing.invalid").expect("valid email"),
            UserType::User,
        ),
    )
    .with_session_key(key)
    .with_ttl(Duration::hours(1))
    .build()
}

#[test]
fn cloud_target_uses_only_the_profile_tenant_and_recovers_after_store_repair() {
    let fixture = isolated_fixture(8080);
    let system_root = fixture.system_dir.join("routing-project");
    let state_dir = system_root.join(".systemprompt");
    let sessions_dir = state_dir.join("sessions");
    let tenants_path = state_dir.join("tenants.json");
    fs::create_dir_all(&state_dir).expect("create owned routing state");
    fs::create_dir_all(system_root.join("services")).expect("mark owned routing project root");

    let mut profile: Profile = serde_yaml::from_str(
        &fs::read_to_string(&fixture.profile_path).expect("read isolated profile"),
    )
    .expect("parse isolated profile");
    let routed_tenant = TenantId::new("tenant_routed");
    let other_tenant = TenantId::new("tenant_other");
    profile.target = ProfileType::Cloud;
    profile.paths.system = "/app".to_owned();
    profile.paths.services = "/app/services".to_owned();
    profile.paths.bin = "/app/bin".to_owned();
    profile.paths.web_path = Some("/app/web".to_owned());
    profile.server.api_external_url = "https://routing.example.invalid".to_owned();
    profile
        .server
        .trusted_proxies
        .push("fc00::/7".parse().expect("valid cloud proxy range"));
    profile.security.issuer = ISSUER.to_owned();
    profile.cloud = Some(CloudConfig {
        tenant_id: Some(routed_tenant.clone()),
        validation: CloudValidationMode::Strict,
    });
    fs::write(
        &fixture.profile_path,
        serde_yaml::to_string(&profile).expect("serialize cloud profile"),
    )
    .expect("write cloud profile");

    let mut routed = StoredTenant::new(routed_tenant.clone(), "Routed".to_owned());
    routed.hostname = Some("routed.example.invalid".to_owned());
    let mut other = StoredTenant::new(other_tenant.clone(), "Other".to_owned());
    other.hostname = Some("other.example.invalid".to_owned());
    TenantStore::new(vec![routed.clone(), other.clone()])
        .save_to_path(&tenants_path)
        .expect("save initial tenant cache");

    let routed_key = SessionKey::Tenant(routed_tenant.clone());
    let other_key = SessionKey::Tenant(other_tenant);
    let routed_context = ContextId::generate();
    let other_context = ContextId::generate();
    let mut sessions = SessionStore::new();
    sessions.upsert_session(
        &routed_key,
        session_for(&routed_key, ISSUER, "routed-token", routed_context.clone()),
    );
    sessions.upsert_session(
        &other_key,
        session_for(&other_key, ISSUER, "other-token", other_context),
    );
    sessions.save(&sessions_dir).expect("save initial sessions");

    let _cwd = WorkingDirectory::enter(&system_root);
    ProfileBootstrap::init_from_path(&fixture.profile_path).expect("initialize cloud profile");

    let initial = determine_execution_target().expect("valid cloud state routes remotely");
    match initial {
        ExecutionTarget::Remote {
            hostname,
            token,
            context,
        } => {
            assert_eq!(hostname, "routed.example.invalid");
            assert_eq!(token.as_str(), "routed-token");
            assert_eq!(context.as_str(), routed_context.as_str());
        },
        ExecutionTarget::Local => panic!("a complete cloud profile must not route locally"),
    }

    TenantStore::new(vec![other.clone()])
        .save_to_path(&tenants_path)
        .expect("remove the selected tenant from the cache");
    let missing = determine_execution_target()
        .expect_err("an unrelated cached account cannot replace the profile tenant");
    let missing_message = format!("{missing:#}");
    assert!(missing_message.contains(routed_tenant.as_str()));
    assert!(!missing_message.contains("other.example.invalid"));

    TenantStore::new(vec![routed, other])
        .save_to_path(&tenants_path)
        .expect("repair tenant cache");
    sessions.upsert_session(
        &routed_key,
        session_for(
            &routed_key,
            "https://old-routing-issuer.test",
            "stale-token",
            ContextId::generate(),
        ),
    );
    sessions
        .save(&sessions_dir)
        .expect("replace selected session with stale issuer");
    let stale =
        determine_execution_target().expect_err("an issuer-mismatched session must fail closed");
    assert!(format!("{stale:#}").contains("admin session login"));

    let repaired_context = ContextId::generate();
    sessions.upsert_session(
        &routed_key,
        session_for(
            &routed_key,
            ISSUER,
            "repaired-token",
            repaired_context.clone(),
        ),
    );
    sessions
        .save(&sessions_dir)
        .expect("repair selected session");
    let repaired =
        determine_execution_target().expect("routing recovers from repaired durable state");
    match repaired {
        ExecutionTarget::Remote {
            hostname,
            token,
            context,
        } => {
            assert_eq!(hostname, "routed.example.invalid");
            assert_eq!(token.as_str(), "repaired-token");
            assert_eq!(context.as_str(), repaired_context.as_str());
        },
        ExecutionTarget::Local => panic!("repaired cloud state must not route locally"),
    }
}
