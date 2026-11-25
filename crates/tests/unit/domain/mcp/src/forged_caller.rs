use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rmcp::ErrorData as McpError;
use rmcp::model::CallToolRequestParams;
use schemars::JsonSchema;
use serde::Deserialize;
use systemprompt_ai::repository::AiRequestRepository;
use systemprompt_extension::mcp::{McpToolHandler, VerifiedCaller, execute};
use systemprompt_identifiers::{
    AgentName, ContextId, JwtToken, McpExecutionId, McpServerId, SessionId, TraceId, UserId,
};
use systemprompt_mcp::repository::ToolUsageRepository;
use systemprompt_mcp::services::ui_renderer::UiRendererRegistry;
use systemprompt_mcp::{ArtifactIngest, McpToolExecutor};
use systemprompt_models::RequestContext;
use systemprompt_models::artifacts::TextArtifact;
use systemprompt_models::mcp::ClientProfile;
use systemprompt_security::jwt::{encode_mcp_hop, mcp_hop_claims};
use systemprompt_test_fixtures::{
    FIXTURE_EXTERNAL_URL, fixture_caller_verification, fixture_hop_identity,
    fixture_verified_caller, test_db_pool, test_signing_keys, test_token_authority,
};

const SERVER: &str = "srv-forged-caller";

#[derive(Debug, Deserialize, JsonSchema)]
struct Empty {}

struct Witness(Arc<AtomicBool>);

impl McpToolHandler for Witness {
    type Input = Empty;
    type Output = TextArtifact;

    fn tool_name(&self) -> &'static str {
        "witness"
    }

    async fn handle(
        &self,
        _input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(Self::Output, String), McpError> {
        self.0.store(true, Ordering::SeqCst);
        Ok((TextArtifact::new("ran".to_owned()), "ran".to_owned()))
    }
}

fn ctx_for(user: &str) -> RequestContext {
    RequestContext::new(
        SessionId::new(format!("s-{user}")),
        TraceId::new("t-forged"),
        ContextId::generate(),
        AgentName::try_new("agent-forged").expect("agent"),
        systemprompt_identifiers::Actor::anonymous(UserId::new(user)),
    )
}

async fn executor() -> McpToolExecutor {
    let db = test_db_pool().await;
    McpToolExecutor::new(
        Arc::new(ToolUsageRepository::new(&db)),
        Some(Arc::new(AiRequestRepository::new(&db))),
        Arc::new(ArtifactIngest::from_db(
            &db,
            systemprompt_governance::policy::secrets::SecretScannerCell::default(),
        )),
        Arc::new(UiRendererRegistry::with_builtins()),
        systemprompt_mcp::ServerCallers {
            server: McpServerId::new(SERVER),
            verification: fixture_caller_verification(),
        },
    )
}

async fn run(caller: &VerifiedCaller) -> (Result<rmcp::model::CallToolResult, McpError>, bool) {
    let exec = executor().await;
    let ran = Arc::new(AtomicBool::new(false));
    let outcome = execute(
        &exec,
        &Witness(Arc::clone(&ran)),
        &CallToolRequestParams::new("witness".to_owned()),
        caller,
        &ClientProfile::unknown(),
    )
    .await;
    (outcome, ran.load(Ordering::SeqCst))
}

fn token_for(server: &str, ctx: &RequestContext, issuer: &str) -> JwtToken {
    JwtToken::new(
        test_token_authority(issuer)
            .mint_mcp_hop(&McpServerId::new(server), &fixture_hop_identity(ctx))
            .expect("mint"),
    )
}

async fn assert_refused(caller: VerifiedCaller) {
    let (outcome, ran) = run(&caller).await;
    assert!(
        outcome.is_err(),
        "a caller that does not verify must be refused"
    );
    assert!(!ran, "the handler must not run for a refused caller");
}

#[tokio::test]
async fn a_caller_whose_token_verifies_runs() {
    let ctx = ctx_for("user-a");
    let (outcome, ran) = run(&fixture_verified_caller(&McpServerId::new(SERVER), &ctx)).await;
    outcome.expect("a verified caller runs");
    assert!(ran);
}

#[tokio::test]
async fn a_caller_with_a_forged_token_is_refused() {
    let ctx = ctx_for("user-a");
    assert_refused(VerifiedCaller::presented(
        JwtToken::new("e30.e30.forged"),
        ctx,
    ))
    .await;
}

#[tokio::test]
async fn a_caller_with_another_servers_token_is_refused() {
    let ctx = ctx_for("user-a");
    let token = token_for("some-other-server", &ctx, FIXTURE_EXTERNAL_URL);
    assert_refused(VerifiedCaller::presented(token, ctx)).await;
}

#[tokio::test]
async fn a_caller_with_another_issuers_token_is_refused() {
    let ctx = ctx_for("user-a");
    let token = token_for(SERVER, &ctx, "https://elsewhere.test");
    assert_refused(VerifiedCaller::presented(token, ctx)).await;
}

#[tokio::test]
async fn a_caller_with_an_expired_token_is_refused() {
    let ctx = ctx_for("user-a");
    let mut claims = mcp_hop_claims(
        FIXTURE_EXTERNAL_URL,
        &McpServerId::new(SERVER),
        &fixture_hop_identity(&ctx),
    );
    claims.registered.iat -= 600;
    claims.registered.nbf = Some(claims.registered.iat);
    claims.registered.exp = claims.registered.iat + 60;
    let token = encode_mcp_hop(&test_signing_keys(), &claims).expect("encode");
    assert_refused(VerifiedCaller::presented(JwtToken::new(token), ctx)).await;
}

#[tokio::test]
async fn a_token_for_user_a_cannot_record_as_user_b() {
    let token = token_for(SERVER, &ctx_for("user-a"), FIXTURE_EXTERNAL_URL);
    assert_refused(VerifiedCaller::presented(token, ctx_for("user-b"))).await;
}

#[tokio::test]
async fn a_token_cannot_carry_another_session() {
    let ctx = ctx_for("user-a");
    let token = token_for(SERVER, &ctx, FIXTURE_EXTERNAL_URL);
    let mut other = ctx;
    other.request.session_id = SessionId::new("s-stolen");
    assert_refused(VerifiedCaller::presented(token, other)).await;
}
