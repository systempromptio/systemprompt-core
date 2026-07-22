//! DB-backed tests for the `core contexts` create/edit/show/delete seams and
//! partial-identifier resolution, driven with a synthetic CLI session.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_cli::CliConfig;
use systemprompt_cli::core::contexts::{create, delete, edit, resolve, show};
use systemprompt_cli::interactive::ScriptedPrompter;
use systemprompt_cloud::{CliSession, SessionIdentity};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContextId, Email, ProfileName, SessionId, SessionToken, UserId};
use systemprompt_models::auth::UserType;
use systemprompt_test_fixtures::{
    fixture_database_url, fixture_db_pool, seed_user_row, seed_user_session, unique_user_id,
};

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().unwrap())
        .await
        .unwrap()
}

async fn seeded_identity(pool: &DbPool, prefix: &str) -> (UserId, SessionId) {
    let user_id = unique_user_id(prefix);
    let email = format!("{}@test.local", user_id.as_str());
    seed_user_row(pool, &user_id, &email).await.unwrap();
    let session_id = SessionId::generate();
    seed_user_session(pool, &user_id, &session_id)
        .await
        .unwrap();
    (user_id, session_id)
}

fn session_for(user_id: &UserId, session_id: SessionId, context_id: ContextId) -> CliSession {
    CliSession::builder(
        ProfileName::new("ctxcmd"),
        SessionToken::new("token"),
        session_id,
        context_id,
        SessionIdentity::new(
            user_id.clone(),
            Email::new("ctxcmd@test.local"),
            UserType::Admin,
        ),
    )
    .build()
}

fn cfg() -> CliConfig {
    CliConfig::new().with_interactive(false)
}

fn card_title(out: &systemprompt_cli::shared::CommandOutput) -> String {
    serde_json::to_value(out.artifact())
        .ok()
        .and_then(|v| v.get("title").and_then(|t| t.as_str()).map(str::to_owned))
        .unwrap_or_default()
}

async fn context_name(pool: &DbPool, user_id: &UserId, context_id: &ContextId) -> String {
    ContextRepository::new(pool)
        .unwrap()
        .list_contexts_basic(user_id)
        .await
        .unwrap()
        .into_iter()
        .find(|c| c.context_id == *context_id)
        .expect("context exists")
        .name
}

#[tokio::test]
async fn create_persists_named_and_default_contexts() {
    let pool = pool().await;
    let (user_id, session_id) = seeded_identity(&pool, "ctxcreate").await;
    let session = session_for(&user_id, session_id, ContextId::generate());

    let named = create::execute_with_pool(
        create::CreateArgs {
            name: Some("named-context".to_owned()),
        },
        &session,
        &pool,
        &cfg(),
    )
    .await
    .unwrap();
    assert_eq!(card_title(&named), "Context Created");

    create::execute_with_pool(create::CreateArgs { name: None }, &session, &pool, &cfg())
        .await
        .unwrap();

    let names: Vec<String> = ContextRepository::new(&pool)
        .unwrap()
        .list_contexts_basic(&user_id)
        .await
        .unwrap()
        .into_iter()
        .map(|c| c.name)
        .collect();
    assert!(names.iter().any(|n| n == "named-context"), "{names:?}");
    assert!(
        names.iter().any(|n| n.starts_with("Context - ")),
        "{names:?}"
    );
}

#[tokio::test]
async fn edit_renames_by_full_id_and_prefix() {
    let pool = pool().await;
    let (user_id, session_id) = seeded_identity(&pool, "ctxedit").await;
    let session = session_for(&user_id, session_id.clone(), ContextId::generate());
    let repo = ContextRepository::new(&pool).unwrap();
    let context_id = repo
        .get_or_create_cli_context(&user_id, &session_id, "edit-me")
        .await
        .unwrap();

    edit::execute_with_pool(
        edit::EditArgs {
            context: context_id.as_str().to_owned(),
            name: "renamed-full".to_owned(),
        },
        &session,
        &pool,
        &cfg(),
    )
    .await
    .unwrap();
    assert_eq!(
        context_name(&pool, &user_id, &context_id).await,
        "renamed-full"
    );

    let prefix = &context_id.as_str()[..8];
    edit::execute_with_pool(
        edit::EditArgs {
            context: prefix.to_owned(),
            name: "renamed-prefix".to_owned(),
        },
        &session,
        &pool,
        &cfg(),
    )
    .await
    .unwrap();
    assert_eq!(
        context_name(&pool, &user_id, &context_id).await,
        "renamed-prefix"
    );
}

#[tokio::test]
async fn resolve_matches_by_name_and_rejects_unknown() {
    let pool = pool().await;
    let (user_id, session_id) = seeded_identity(&pool, "ctxres").await;
    let repo = ContextRepository::new(&pool).unwrap();
    let context_id = repo
        .get_or_create_cli_context(&user_id, &session_id, "Resolve Target")
        .await
        .unwrap();

    let by_name = resolve::resolve_context("Resolve Target", &user_id, &repo)
        .await
        .unwrap();
    assert_eq!(by_name, context_id);

    let by_case_insensitive_name = resolve::resolve_context("resolve target", &user_id, &repo)
        .await
        .unwrap();
    assert_eq!(by_case_insensitive_name, context_id);

    let err = resolve::resolve_context("does-not-exist", &user_id, &repo)
        .await
        .expect_err("unknown identifier");
    assert!(err.to_string().contains("Context not found"), "{err}");
}

#[tokio::test]
async fn show_reports_active_flag_for_session_context() {
    let pool = pool().await;
    let (user_id, session_id) = seeded_identity(&pool, "ctxshow").await;
    let repo = ContextRepository::new(&pool).unwrap();
    let context_id = repo
        .get_or_create_cli_context(&user_id, &session_id, "show-me")
        .await
        .unwrap();
    let session = session_for(&user_id, session_id, context_id.clone());

    let out = show::execute_with_pool(
        show::ShowArgs {
            context: context_id.as_str().to_owned(),
        },
        &session,
        &pool,
        &cfg(),
    )
    .await
    .unwrap();
    assert_eq!(card_title(&out), "Context Details");
    let raw = serde_json::to_string(out.artifact()).unwrap();
    assert!(raw.contains("is_active"), "{raw}");
    assert!(raw.contains("show-me"), "{raw}");
}

#[tokio::test]
async fn delete_refuses_active_context_and_removes_inactive_one() {
    let pool = pool().await;
    let (user_id, session_id) = seeded_identity(&pool, "ctxdel").await;
    let repo = ContextRepository::new(&pool).unwrap();
    let active = repo
        .get_or_create_cli_context(&user_id, &session_id, "active-ctx")
        .await
        .unwrap();
    let victim = repo
        .create_context(
            &user_id,
            Some(&session_id),
            "victim-ctx",
            systemprompt_agent::models::context::ContextKind::User,
        )
        .await
        .unwrap();
    let session = session_for(&user_id, session_id, active.clone());
    let prompter = ScriptedPrompter::new(std::iter::empty::<String>());

    let err = delete::execute_with_pool(
        delete::DeleteArgs {
            context: active.as_str().to_owned(),
            yes: false,
        },
        &session,
        &pool,
        &cfg(),
        &prompter,
    )
    .await
    .expect_err("active context must not be deletable");
    assert!(err.to_string().contains("active context"), "{err}");

    let out = delete::execute_with_pool(
        delete::DeleteArgs {
            context: victim.as_str().to_owned(),
            yes: true,
        },
        &session,
        &pool,
        &cfg(),
        &prompter,
    )
    .await
    .unwrap();
    assert_eq!(card_title(&out), "Context Deleted");

    let remaining: Vec<ContextId> = repo
        .list_contexts_basic(&user_id)
        .await
        .unwrap()
        .into_iter()
        .map(|c| c.context_id)
        .collect();
    assert!(!remaining.contains(&victim), "{remaining:?}");
}

#[tokio::test]
async fn delete_cancellation_keeps_the_context() {
    let pool = pool().await;
    let (user_id, session_id) = seeded_identity(&pool, "ctxcancel").await;
    let repo = ContextRepository::new(&pool).unwrap();
    let active = repo
        .get_or_create_cli_context(&user_id, &session_id, "cancel-active")
        .await
        .unwrap();
    let victim = repo
        .create_context(
            &user_id,
            Some(&session_id),
            "cancel-victim",
            systemprompt_agent::models::context::ContextKind::User,
        )
        .await
        .unwrap();
    let session = session_for(&user_id, session_id, active);
    let interactive = CliConfig::new()
        .with_interactive(true)
        .with_assume_terminal(true);
    let prompter = ScriptedPrompter::new(["n".to_owned()]);

    let out = delete::execute_with_pool(
        delete::DeleteArgs {
            context: victim.as_str().to_owned(),
            yes: false,
        },
        &session,
        &pool,
        &interactive,
        &prompter,
    )
    .await
    .unwrap();
    assert_eq!(card_title(&out), "Context Delete Cancelled");

    let remaining: Vec<ContextId> = repo
        .list_contexts_basic(&user_id)
        .await
        .unwrap()
        .into_iter()
        .map(|c| c.context_id)
        .collect();
    assert!(remaining.contains(&victim), "{remaining:?}");
}

fn minimal_profile() -> systemprompt_models::Profile {
    use systemprompt_models::auth::JwtAudience;
    use systemprompt_models::services::SystemAdminConfig;
    use systemprompt_models::{
        ContentNegotiationConfig, ExtensionsConfig, PathsConfig, Profile, ProfileDatabaseConfig,
        ProfileType, RateLimitsConfig, RuntimeConfig, SecurityConfig, SecurityHeadersConfig,
        ServerConfig, SiteConfig,
    };

    Profile {
        name: "ctxcmd".to_string(),
        display_name: "Ctx".to_string(),
        target: ProfileType::Local,
        site: SiteConfig {
            name: "Test Site".to_string(),
            github_link: None,
        },
        database: ProfileDatabaseConfig {
            db_type: "postgres".to_string(),
            external_db_access: false,
            pool: None,
        },
        server: ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
            api_server_url: "http://localhost:8080".to_string(),
            api_internal_url: "http://localhost:8080".to_string(),
            api_external_url: "https://example.com".to_string(),
            use_https: false,
            cors_allowed_origins: vec![],
            content_negotiation: ContentNegotiationConfig::default(),
            security_headers: SecurityHeadersConfig::default(),
            instance_id: None,
            max_concurrent_streams: systemprompt_models::config::DEFAULT_MAX_CONCURRENT_STREAMS,
            trusted_proxies: Vec::new(),
        },
        paths: PathsConfig {
            system: "/tmp/ctxcmd".to_string(),
            services: "/tmp/ctxcmd/services".to_string(),
            bin: "/tmp/ctxcmd/bin".to_string(),
            web_path: None,
            storage: None,
            geoip_database: None,
        },
        security: SecurityConfig {
            issuer: "https://issuer.test".to_string(),
            access_token_expiration: 3600,
            refresh_token_expiration: 86400,
            audiences: vec![JwtAudience::Api],
            allowed_resource_audiences: vec![],
            allow_registration: true,
            signing_key_path: std::path::PathBuf::from("/tmp/test-signing-key.pem"),
            trusted_issuers: vec![],
            id_jag_ttl_secs: systemprompt_models::profile::DEFAULT_ID_JAG_TTL_SECS,
        },
        rate_limits: RateLimitsConfig::default(),
        runtime: RuntimeConfig::default(),
        cloud: None,
        secrets: None,
        extensions: ExtensionsConfig::default(),
        providers: systemprompt_models::profile::ProviderRegistry::default(),
        gateway: None,
        governance: None,
        system_admin: SystemAdminConfig {
            username: "admin".to_string(),
        },
    }
}

#[tokio::test]
async fn new_execute_resolved_creates_context_and_updates_session_store() {
    use systemprompt_cli::core::contexts::new;
    use systemprompt_cli::session::CliSessionContext;
    use systemprompt_cloud::{SessionKey, SessionStore};

    let pool = pool().await;
    let (user_id, session_id) = seeded_identity(&pool, "ctxnew").await;
    let session_ctx = CliSessionContext {
        session: session_for(&user_id, session_id, ContextId::generate()),
        profile: minimal_profile(),
    };
    let sessions = tempfile::tempdir().unwrap();

    let out = new::execute_resolved(
        new::NewArgs {
            name: Some("seam-context".to_owned()),
        },
        &cfg(),
        &session_ctx,
        &pool,
        sessions.path(),
    )
    .await
    .unwrap();
    assert_eq!(card_title(&out), "New Context Created");

    let names: Vec<String> = ContextRepository::new(&pool)
        .unwrap()
        .list_contexts_basic(&user_id)
        .await
        .unwrap()
        .into_iter()
        .map(|c| c.name)
        .collect();
    assert!(names.contains(&"seam-context".to_owned()), "{names:?}");

    let store = SessionStore::load_or_create(sessions.path()).unwrap();
    let stored = store
        .get_session(&SessionKey::from_tenant_id(None))
        .expect("session persisted");
    assert_ne!(stored.context_id, *session_ctx.context_id());

    let defaulted = new::execute_resolved(
        new::NewArgs { name: None },
        &cfg(),
        &session_ctx,
        &pool,
        sessions.path(),
    )
    .await
    .unwrap();
    assert_eq!(card_title(&defaulted), "New Context Created");
}

#[tokio::test]
async fn use_execute_resolved_switches_to_named_context() {
    use systemprompt_cli::core::contexts::use_context;
    use systemprompt_cli::session::CliSessionContext;
    use systemprompt_cloud::{SessionKey, SessionStore};

    let pool = pool().await;
    let (user_id, session_id) = seeded_identity(&pool, "ctxuse").await;
    let repo = ContextRepository::new(&pool).unwrap();
    let target = repo
        .create_context(
            &user_id,
            Some(&session_id),
            "switch-target",
            systemprompt_agent::models::context::ContextKind::User,
        )
        .await
        .unwrap();

    let session_ctx = CliSessionContext {
        session: session_for(&user_id, session_id, ContextId::generate()),
        profile: minimal_profile(),
    };
    let sessions = tempfile::tempdir().unwrap();

    let out = use_context::execute_resolved(
        use_context::UseArgs {
            context: "switch-target".to_owned(),
        },
        &cfg(),
        &session_ctx,
        &pool,
        sessions.path(),
    )
    .await
    .unwrap();
    assert_eq!(card_title(&out), "Context Switched");

    let store = SessionStore::load_or_create(sessions.path()).unwrap();
    let stored = store
        .get_session(&SessionKey::from_tenant_id(None))
        .expect("session persisted");
    assert_eq!(stored.context_id, target);

    let missing = use_context::execute_resolved(
        use_context::UseArgs {
            context: "no-such-context".to_owned(),
        },
        &cfg(),
        &session_ctx,
        &pool,
        sessions.path(),
    )
    .await;
    assert!(missing.is_err());
}
