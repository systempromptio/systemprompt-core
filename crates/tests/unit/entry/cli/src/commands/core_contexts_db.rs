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
