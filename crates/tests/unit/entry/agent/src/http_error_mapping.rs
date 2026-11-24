//! `AgentHttpError`: the agent routes' failures keep their status through the
//! API error model — a missing entity is 404, a stale write 409, corrupt
//! stored data and an unclassified agent failure 500.

use axum::http::StatusCode;
use axum::response::IntoResponse;
use systemprompt_agent::AgentError;
use systemprompt_agent_entry::http::AgentHttpError;
use systemprompt_traits::RepositoryError;

fn status_of(err: AgentError) -> StatusCode {
    AgentHttpError::from(err).into_response().status()
}

#[test]
fn agent_error_not_found_and_validation_are_distinguished_from_the_catch_all() {
    assert_eq!(
        status_of(AgentError::NotFound {
            name: "a".to_owned(),
        }),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        status_of(AgentError::EmptyCorsAllowlist),
        StatusCode::INTERNAL_SERVER_ERROR,
        "an unclassified agent failure must not be reported as a client error"
    );
}

#[test]
fn agent_repository_errors_keep_their_classification() {
    assert_eq!(
        status_of(AgentError::Repository(RepositoryError::not_found(
            "task", "t1"
        ))),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        status_of(AgentError::Repository(RepositoryError::conflict(
            "task",
            "t1",
            "stale version"
        ))),
        StatusCode::CONFLICT
    );
}

#[test]
fn a_stored_task_without_an_agent_name_is_a_server_error() {
    let err = AgentError::Repository(RepositoryError::invalid_data(
        "agent_name",
        "missing for task t1",
    ));
    assert_eq!(
        status_of(err),
        StatusCode::INTERNAL_SERVER_ERROR,
        "corrupt stored data is the server's fault, not the caller's"
    );
}
