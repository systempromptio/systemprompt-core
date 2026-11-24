//! End-to-end coverage for the shared messaging dispatch pipeline.
//!
//! `A2aMessagingDispatcher::run` is driven directly with a fixture
//! `AppContext`, a real Postgres pool, the process test signing key, and a
//! wiremock agent backend seeded as a running `agent` service. The agent's
//! `oauth.required = false` (fixture `config.yaml`) lets the proxy forward the
//! minted A2A token without a check, so the full path — federated identity,
//! authz hook, token mint, blocking `SendMessage`, reply extraction — runs
//! against live infrastructure.


use systemprompt_agent_entry::messaging::{
    A2aMessagingDispatcher, DispatcherInputs, MessagingError,
};
use systemprompt_api::services::agent_dispatch::{DispatcherBindError, bind_dispatchers};
use systemprompt_governance::authz::{AuthzHookCell, DenyAllHook};
use systemprompt_identifiers::{AgentName, SlackChannelId, SlackUserId, SlackWorkspaceId};
use systemprompt_models::a2a::dispatch::AgentDispatchCell;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    TEST_SLACK_WORKSPACE_ID, agent_error_response_json, agent_reply_response_json,
    ensure_messaging_bootstrap, fixture_app_context_with_hook, seed_agent_backend,
    test_app_context, test_db_pool, test_messaging_agent,
};
use systemprompt_traits::{
    DispatchOutcome, LinkPolicy, MessagingConversation, MessagingInbound, SenderIdentity,
};
use uuid::Uuid;
use wiremock::matchers::{header, method};
use wiremock::{Mock, MockServer, ResponseTemplate};

pub(crate) const ISSUER: &str = "https://slack.com";

pub(crate) fn inbound(external_user_id: &str, text: &str) -> MessagingInbound {
    MessagingInbound {
        issuer: ISSUER.to_owned(),
        conversation: MessagingConversation::Slack {
            workspace_id: SlackWorkspaceId::new(TEST_SLACK_WORKSPACE_ID),
            channel_id: Some(SlackChannelId::new("C_TEST")),
            user_id: SlackUserId::new(external_user_id),
        },
        text: text.to_owned(),
        agent_name: AgentName::try_new(test_messaging_agent()).expect("valid AgentName"),
        sender: SenderIdentity::Unlinked,
        link: LinkPolicy::default(),
    }
}

pub(crate) async fn dispatch_messaging(
    ctx: &AppContext,
    inbound: MessagingInbound,
) -> Result<DispatchOutcome, MessagingError> {
    match bind_dispatchers(ctx) {
        Ok(()) | Err(DispatcherBindError::BindersTaken) => {},
        Err(error) => panic!("the dispatcher cells bind: {error}"),
    }
    dispatcher(ctx).run(inbound).await
}

pub(crate) fn dispatcher(ctx: &AppContext) -> A2aMessagingDispatcher {
    let state = ctx.kernel().state();
    A2aMessagingDispatcher::new(DispatcherInputs {
        users: systemprompt_test_fixtures::user_repository(&ctx),
        authz: state
            .find::<AuthzHookCell>()
            .cloned()
            .expect("the host seeds the authz hook cell"),
        issuer: ctx.config().jwt_issuer.clone(),
        keys: ctx.signing_keys().clone(),
        dispatch: state
            .find::<AgentDispatchCell>()
            .cloned()
            .expect("the API host seeds the agent dispatch cell"),
    })
}

pub(crate) async fn mount_reply(mock: &MockServer, text: &str) {
    Mock::given(method("POST"))
        .and(header("A2A-Version", "1.0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(agent_reply_response_json(text)))
        .mount(mock)
        .await;
}

#[tokio::test]
async fn allow_yields_the_agents_reply_text() -> anyhow::Result<()> {
    let b = ensure_messaging_bootstrap();
    let ctx = test_app_context(&b.database_url).await;

    let backend = MockServer::start().await;
    mount_reply(&backend, "the agent replied").await;
    seed_agent_backend(&ctx, &backend).await?;

    let user = format!("U_{}", Uuid::new_v4().simple());
    let outcome = dispatch_messaging(&ctx, inbound(&user, "hi"))
        .await
        .map_err(|e| anyhow::anyhow!("dispatch failed: {e}"))?;
    match outcome {
        DispatchOutcome::Replied(text) => assert_eq!(text, "the agent replied"),
        DispatchOutcome::Denied(reason) => panic!("expected Replied, got Denied({reason})"),
    }
    Ok(())
}

#[tokio::test]
async fn an_unbound_agent_dispatch_refuses_typed_and_sends_nothing() -> anyhow::Result<()> {
    let b = ensure_messaging_bootstrap();
    let ctx = test_app_context(&b.database_url).await;

    let backend = MockServer::start().await;
    mount_reply(&backend, "never sent").await;
    seed_agent_backend(&ctx, &backend).await?;

    let user = format!("U_{}", Uuid::new_v4().simple());
    let error = dispatcher(&ctx)
        .run(inbound(&user, "hi"))
        .await
        .expect_err("no agent dispatch is bound");
    assert!(
        matches!(error, MessagingError::DispatchUnbound),
        "an unbound dispatch fails typed, got {error:?}"
    );
    assert!(
        backend
            .received_requests()
            .await
            .unwrap_or_default()
            .is_empty(),
        "nothing reaches the agent"
    );
    Ok(())
}

#[tokio::test]
async fn deny_hook_short_circuits_to_denied() -> anyhow::Result<()> {
    let b = ensure_messaging_bootstrap();
    let ctx =
        fixture_app_context_with_hook(&b.database_url, std::sync::Arc::new(DenyAllHook::null()))
            .await?;

    let user = format!("U_{}", Uuid::new_v4().simple());
    let outcome = dispatch_messaging(&ctx, inbound(&user, "hi"))
        .await
        .map_err(|e| anyhow::anyhow!("dispatch failed: {e}"))?;
    assert!(
        matches!(outcome, DispatchOutcome::Denied(_)),
        "deny-all hook must short-circuit to Denied, got {outcome:?}"
    );
    Ok(())
}

#[tokio::test]
async fn agent_json_rpc_error_surfaces_as_dispatch_error() -> anyhow::Result<()> {
    let b = ensure_messaging_bootstrap();
    let ctx = test_app_context(&b.database_url).await;

    let backend = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(agent_error_response_json(-32603, "internal agent error")),
        )
        .mount(&backend)
        .await;
    seed_agent_backend(&ctx, &backend).await?;

    let user = format!("U_{}", Uuid::new_v4().simple());
    let err = dispatch_messaging(&ctx, inbound(&user, "hi"))
        .await
        .expect_err("a JSON-RPC error response is a dispatch failure");
    assert!(
        matches!(err, MessagingError::AgentRejected { .. }),
        "expected AgentRejected, got {err:?}"
    );
    Ok(())
}

#[tokio::test]
async fn first_contact_creates_a_federated_user_reused_on_the_second_call() -> anyhow::Result<()> {
    let b = ensure_messaging_bootstrap();
    let pool = test_db_pool().await;
    let ctx = test_app_context(&b.database_url).await;

    let backend = MockServer::start().await;
    mount_reply(&backend, "ok").await;
    seed_agent_backend(&ctx, &backend).await?;

    let user = format!("U_{}", Uuid::new_v4().simple());
    let pg = pool.pool();

    dispatch_messaging(&ctx, inbound(&user, "first"))
        .await
        .map_err(|e| anyhow::anyhow!("dispatch failed: {e}"))?;
    let first: String = sqlx::query_scalar(
        "SELECT user_id FROM federated_identities WHERE issuer=$1 AND external_sub=$2",
    )
    .bind(ISSUER)
    .bind(&user)
    .fetch_one(pg.as_ref())
    .await?;

    dispatch_messaging(&ctx, inbound(&user, "second"))
        .await
        .map_err(|e| anyhow::anyhow!("dispatch failed: {e}"))?;
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

#[tokio::test]
async fn the_dispatcher_cells_bind_once_and_a_second_bind_is_refused() -> anyhow::Result<()> {
    let b = ensure_messaging_bootstrap();
    let ctx = test_app_context(&b.database_url).await;
    bind_dispatchers(&ctx)?;
    let second = bind_dispatchers(&ctx).expect_err("the binders are taken by the first bind");
    assert!(
        matches!(second, DispatcherBindError::BindersTaken),
        "a second bind fails typed, got {second:?}"
    );
    Ok(())
}
