//! The document actor a request runs as, from its credential alone.

use systemprompt_files::documents::agents::DocumentActor;
use systemprompt_identifiers::{Actor, AgentName, ClientId, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::execution::context::{RequestContext, UserInteractionMode};
use systemprompt_runtime::documents::actor::request_actor;

fn context() -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::try_new("header-name").expect("agent name"),
        Actor::user(UserId::new("actor-user")),
    )
}

#[test]
fn only_a_browser_session_is_the_person() {
    assert_eq!(
        request_actor(&context().with_browser_session()),
        DocumentActor::Human
    );
    assert_eq!(
        request_actor(&context()),
        DocumentActor::agent(AgentName::new("agent"), false)
    );
}

#[test]
fn a_browser_session_with_a_purpose_or_delegation_is_an_agent() {
    let purposed = context()
        .with_browser_session()
        .with_caller_purpose("farm:run-7".to_owned());
    assert_eq!(
        request_actor(&purposed),
        DocumentActor::agent(AgentName::new("farm:run-7"), false)
    );
    let delegated = context()
        .with_browser_session()
        .with_act_chain(vec![Actor::user(UserId::new("delegate"))]);
    assert!(request_actor(&delegated).is_agent());
}

#[test]
fn the_name_and_attendance_come_from_signed_claims_not_headers() {
    let bridge = context()
        .with_client_id(ClientId::bridge())
        .with_interaction_mode(UserInteractionMode::Interactive);
    let DocumentActor::Agent(agent) = request_actor(&bridge) else {
        panic!("a bearer token is an agent");
    };
    assert_eq!(agent.name, ClientId::bridge().as_str());
    assert!(agent.attended);
    assert_ne!(agent.name, "header-name");
}

#[test]
fn a_token_with_a_purpose_is_never_attended() {
    let purposed = context()
        .with_client_id(ClientId::bridge())
        .with_interaction_mode(UserInteractionMode::Interactive)
        .with_caller_purpose("broker:svc".to_owned());
    let DocumentActor::Agent(agent) = request_actor(&purposed) else {
        panic!("a bearer token is an agent");
    };
    assert_eq!(agent.name, "broker:svc");
    assert!(!agent.attended);
}

#[tokio::test]
async fn an_extension_runs_its_document_calls_as_the_request_actor() {
    use systemprompt_files::documents::agents::current_actor;
    use systemprompt_runtime::documents::actor::act_as_request;

    let browser = context().with_browser_session();
    let inside = act_as_request(&browser, async { current_actor() }).await;
    assert_eq!(inside, DocumentActor::Human);
    let bearer = context().with_client_id(ClientId::bridge());
    let inside = act_as_request(&bearer, async { current_actor() }).await;
    assert!(inside.is_agent());
}

#[test]
fn a_purpose_that_is_not_a_valid_agent_name_falls_back_to_the_unnamed_agent() {
    for purpose in ["has space", "unknown", &"x".repeat(65)] {
        let purposed = context().with_caller_purpose(purpose.to_owned());
        let DocumentActor::Agent(agent) = request_actor(&purposed) else {
            panic!("a bearer token is an agent");
        };
        assert_eq!(agent.name, "agent", "{purpose}");
    }
}
