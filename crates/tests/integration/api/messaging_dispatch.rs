//! End-to-end coverage for the shared messaging dispatch pipeline.
//!
//! `dispatch_messaging` is driven directly with a fixture `AppContext`, a real
//! Postgres pool, the process test signing key, and a wiremock agent backend
//! seeded as a running `agent` service. The agent's `oauth.required = false`
//! (fixture `config.yaml`) lets the proxy forward the minted A2A token without
//! a check, so the full path — federated identity, authz hook, token mint,
//! blocking `message/send`, reply extraction — runs against live
//! infrastructure.

use systemprompt_api::routes::messaging::{
    DispatchOutcome, MessagingError, MessagingInbound, ReplyTarget, dispatch_messaging,
};
use systemprompt_identifiers::{AgentName, SlackWorkspaceId};
use systemprompt_security::authz::{DenyAllHook, EntityRef};
use systemprompt_test_fixtures::{
    TEST_MESSAGING_AGENT, TEST_SLACK_WORKSPACE_ID, agent_error_response_json,
    agent_reply_response_json, ensure_test_bootstrap, fixture_app_context,
    fixture_app_context_with_hook, fixture_db_pool, install_test_signing_key, seed_agent_backend,
};
use uuid::Uuid;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

const ISSUER: &str = "https://slack.com";

fn inbound(external_user_id: &str, text: &str) -> MessagingInbound {
    MessagingInbound {
        platform: "slack",
        issuer: ISSUER.to_owned(),
        org_id: TEST_SLACK_WORKSPACE_ID.to_owned(),
        channel_id: "C_TEST".to_owned(),
        external_user_id: external_user_id.to_owned(),
        text: text.to_owned(),
        agent_name: AgentName::new(TEST_MESSAGING_AGENT),
        entity: EntityRef::SlackWorkspace(SlackWorkspaceId::new(TEST_SLACK_WORKSPACE_ID)),
        reply: ReplyTarget::Channel {
            id: "C_TEST".to_owned(),
        },
    }
}

async fn mount_reply(mock: &MockServer, text: &str) {
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(agent_reply_response_json(text)))
        .mount(mock)
        .await;
}

#[tokio::test]
async fn allow_yields_the_agents_reply_text() -> anyhow::Result<()> {
    let b = ensure_test_bootstrap();
    install_test_signing_key();
    let pool = fixture_db_pool(&b.database_url).await?;
    let ctx = fixture_app_context(&pool, &b.database_url)?;

    let backend = MockServer::start().await;
    mount_reply(&backend, "the agent replied").await;
    seed_agent_backend(&pool, &backend).await?;

    let user = format!("U_{}", Uuid::new_v4().simple());
    let outcome = dispatch_messaging(&ctx, inbound(&user, "hi")).await?;
    match outcome {
        DispatchOutcome::Replied(text) => assert_eq!(text, "the agent replied"),
        DispatchOutcome::Denied(reason) => panic!("expected Replied, got Denied({reason})"),
    }
    Ok(())
}

#[tokio::test]
async fn deny_hook_short_circuits_to_denied() -> anyhow::Result<()> {
    let b = ensure_test_bootstrap();
    install_test_signing_key();
    let pool = fixture_db_pool(&b.database_url).await?;
    let ctx = fixture_app_context_with_hook(
        &pool,
        &b.database_url,
        std::sync::Arc::new(DenyAllHook::null()),
    )?;

    let user = format!("U_{}", Uuid::new_v4().simple());
    let outcome = dispatch_messaging(&ctx, inbound(&user, "hi")).await?;
    assert!(
        matches!(outcome, DispatchOutcome::Denied(_)),
        "deny-all hook must short-circuit to Denied, got {outcome:?}"
    );
    Ok(())
}

#[tokio::test]
async fn agent_json_rpc_error_surfaces_as_dispatch_error() -> anyhow::Result<()> {
    let b = ensure_test_bootstrap();
    install_test_signing_key();
    let pool = fixture_db_pool(&b.database_url).await?;
    let ctx = fixture_app_context(&pool, &b.database_url)?;

    let backend = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(agent_error_response_json(-32603, "internal agent error")),
        )
        .mount(&backend)
        .await;
    seed_agent_backend(&pool, &backend).await?;

    let user = format!("U_{}", Uuid::new_v4().simple());
    let err = dispatch_messaging(&ctx, inbound(&user, "hi"))
        .await
        .expect_err("a JSON-RPC error response is a dispatch failure");
    assert!(
        matches!(err, MessagingError::Dispatch(_)),
        "expected Dispatch, got {err:?}"
    );
    Ok(())
}

#[tokio::test]
async fn first_contact_creates_a_federated_user_reused_on_the_second_call() -> anyhow::Result<()> {
    let b = ensure_test_bootstrap();
    install_test_signing_key();
    let pool = fixture_db_pool(&b.database_url).await?;
    let ctx = fixture_app_context(&pool, &b.database_url)?;

    let backend = MockServer::start().await;
    mount_reply(&backend, "ok").await;
    seed_agent_backend(&pool, &backend).await?;

    let user = format!("U_{}", Uuid::new_v4().simple());
    let pg = pool.pool_arc().expect("read pool");

    dispatch_messaging(&ctx, inbound(&user, "first")).await?;
    let first: String = sqlx::query_scalar(
        "SELECT user_id FROM federated_identities WHERE issuer=$1 AND external_sub=$2",
    )
    .bind(ISSUER)
    .bind(&user)
    .fetch_one(pg.as_ref())
    .await?;

    dispatch_messaging(&ctx, inbound(&user, "second")).await?;
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT user_id FROM federated_identities WHERE issuer=$1 AND external_sub=$2",
    )
    .bind(ISSUER)
    .bind(&user)
    .fetch_all(pg.as_ref())
    .await?;

    assert_eq!(rows.len(), 1, "the second contact reuses the federated row");
    assert_eq!(rows[0], first, "same governed user id on re-contact");
    Ok(())
}
