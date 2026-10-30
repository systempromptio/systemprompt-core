//! The recall principal names the request's project from its attribution,
//! so recall can search project-scoped memory.

use systemprompt_gateway::audit::GatewayRequestContext;
use systemprompt_gateway::service::stages::recall_principal;
use systemprompt_governance::policy::types::AccessScope;
use systemprompt_identifiers::{AiRequestId, ContextId, ScopeDimension, UserId};
use systemprompt_models::attribution::{AttributionEntry, AttributionSource, RequestAttribution};
use systemprompt_models::auth::RequestActor;
use systemprompt_models::origin::{
    ClientAttestation, ClientEvidence, ClientKind, InboundWireProtocol, MemoryPreference,
    RequestOrigin,
};

fn ctx(attribution: RequestAttribution) -> GatewayRequestContext {
    GatewayRequestContext {
        ai_request_id: AiRequestId::generate(),
        user_id: UserId::new("user-1"),
        session_id: None,
        context_id: ContextId::generate(),
        gateway_conversation_id: None,
        client_session_id: None,
        trace_id: None,
        access_scope: AccessScope::Unknown,
        client_id: None,
        provider: "anthropic".to_owned(),
        hop: None,
        requested_model: None,
        model: "claude-test".to_owned(),
        max_tokens: Some(16),
        is_streaming: false,
        origin: RequestOrigin::gateway(
            ClientKind::ClaudeCode,
            InboundWireProtocol::AnthropicMessages,
            ClientAttestation::None,
        ),
        evidence: ClientEvidence::none(),
        attribution,
        api_key_windows: Vec::new(),
        access_log: None,
        context_bound: false,
        actor: RequestActor::Human,
        memory: MemoryPreference::Default,
    }
}

fn project_attribution(project: &str) -> RequestAttribution {
    RequestAttribution {
        entries: vec![AttributionEntry {
            dimension: ScopeDimension::try_new("project").expect("dimension"),
            value: project.to_owned(),
            source: AttributionSource::Header,
        }],
        ..RequestAttribution::none()
    }
}

#[test]
fn a_project_attribution_names_the_recall_project() {
    let principal = recall_principal(&ctx(project_attribution("atlas")));

    assert_eq!(principal.project.as_deref(), Some("atlas"));
}

#[test]
fn no_project_attribution_leaves_the_project_unset() {
    let principal = recall_principal(&ctx(RequestAttribution::none()));

    assert_eq!(principal.project, None);
}
