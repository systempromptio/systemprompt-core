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
        DocumentActor::agent("agent", false)
    );
}

#[test]
fn a_browser_session_with_a_purpose_or_delegation_is_an_agent() {
    let purposed = context()
        .with_browser_session()
        .with_caller_purpose("farm:run-7".to_owned());
    assert_eq!(
        request_actor(&purposed),
        DocumentActor::agent("farm:run-7", false)
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
