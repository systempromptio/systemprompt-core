//! The execution-capability principal: an evaluation worker calling the
//! gateway on a user's behalf.
//!
//! This credential is machine-issued, so it must not inherit the trust a
//! human's JWT carries. It has no attribute bag to feed authz with, no OAuth
//! client, and its delegation must be visible in the act chain — an evaluation
//! run billed to an owner has to be distinguishable from that owner calling the
//! gateway directly. Its session binding is the execution's own session, not a
//! header the worker supplies.

use axum::http::StatusCode;
use systemprompt_api::routes::gateway::messages::auth::{AuthedPrincipal, ExecutionPrincipal};
use systemprompt_evaluation::repository::experiments::{
    ExecutionIdentity, ExecutionPrincipal as CapabilityPrincipal,
};
use systemprompt_identifiers::{EvalExecutionId, SessionId, TraceId, UserId};
use systemprompt_security::policy::types::AccessScope;

fn execution_principal(session: &SessionId, roles: Vec<String>) -> AuthedPrincipal {
    let identity =
        ExecutionIdentity::builder(UserId::new("owner-1"), EvalExecutionId::new("exec-1"))
            .roles(roles)
            .build();
    AuthedPrincipal::Execution(ExecutionPrincipal {
        principal: CapabilityPrincipal {
            identity,
            session_id: session.clone(),
        },
        trace_id: TraceId::new("trace-exec"),
    })
}

#[test]
fn the_execution_acts_as_the_experiment_owner_not_the_worker() {
    let session = SessionId::generate();
    let principal = execution_principal(&session, vec!["user".to_owned()]);

    assert_eq!(
        principal.user_id().as_str(),
        "owner-1",
        "usage and quota must land on the owner the execution is billed to"
    );
    assert_eq!(principal.trace_id().as_str(), "trace-exec");
    assert_eq!(principal.attested_session(), &session);
}

#[test]
fn the_access_scope_comes_from_the_executions_own_roles() {
    let session = SessionId::generate();

    assert_eq!(
        execution_principal(&session, vec!["user".to_owned()]).access_scope(),
        AccessScope::User
    );
    assert_eq!(
        execution_principal(&session, Vec::new()).access_scope(),
        AccessScope::Unknown,
        "a capability carrying no roles must not fall back to a privileged scope"
    );
}

#[test]
fn a_worker_cannot_smuggle_admin_attributes_into_authz() {
    let session = SessionId::generate();
    let (roles, attributes, act_chain) =
        execution_principal(&session, vec!["user".to_owned()]).authz_attributes();

    assert_eq!(roles, vec!["user".to_owned()]);
    assert!(
        attributes.is_empty(),
        "an execution capability has no attribute bag; authz rules keyed on \
         attributes must see nothing rather than an inherited set"
    );
    assert_eq!(act_chain.len(), 1, "the delegation must be recorded");
}

#[test]
fn the_act_chain_names_the_execution_that_delegated() {
    let session = SessionId::generate();
    let (_, _, act_chain) =
        execution_principal(&session, vec!["user".to_owned()]).authz_attributes();

    let rendered = format!("{:?}", act_chain[0]);
    assert!(
        rendered.contains("exec-1"),
        "an audit reader must be able to trace the call back to its execution: {rendered}"
    );
    assert!(
        rendered.contains("evaluation"),
        "the delegation must be marked as an evaluation job, not a user action: {rendered}"
    );
}

#[test]
fn an_execution_principal_carries_no_oauth_client() {
    let session = SessionId::generate();

    assert!(
        execution_principal(&session, vec!["user".to_owned()])
            .client_id()
            .is_none(),
        "client-scoped policy must not apply to a machine capability"
    );
}

#[test]
fn the_session_binding_is_the_executions_session() {
    let session = SessionId::generate();
    let principal = execution_principal(&session, vec!["user".to_owned()]);

    principal
        .enforce_session_binding(&session)
        .expect("the execution's own session must be accepted");

    let (status, message) = principal
        .enforce_session_binding(&SessionId::generate())
        .expect_err("a worker must not rebind its capability to another session");
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(message.contains("X-Session-ID"));
}
