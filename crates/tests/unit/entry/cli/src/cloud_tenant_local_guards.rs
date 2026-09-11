//! Guards that run before `cloud tenant create --local` touches Docker.
//!
//! Both flows read their answers from the operator and must fail on a bad one
//! before any container is started or any external database is contacted: a
//! container left running behind a rejected answer is an orphan nobody goes
//! looking for.

use systemprompt_cli::ScriptedPrompter;
use systemprompt_cli::cloud::profile::templates::validate_connection;
use systemprompt_cli::cloud::tenant::{create_external_tenant, create_local_tenant};

#[tokio::test]
async fn a_non_numeric_port_is_rejected_before_any_container_is_started() {
    let prompter = ScriptedPrompter::new(["coverage-tenant", "not-a-port"]);

    let error = create_local_tenant(&prompter)
        .await
        .map(|_| ())
        .expect_err("a port that is not a number cannot be bound");

    assert!(
        error
            .to_string()
            .contains("PostgreSQL port must be a number"),
        "the operator must be told which answer was wrong, got: {error}"
    );
}

#[tokio::test]
async fn an_empty_external_database_url_is_rejected_before_validation() {
    let prompter = ScriptedPrompter::new(["coverage-tenant", ""]);

    let error = create_external_tenant(&prompter)
        .await
        .map(|_| ())
        .expect_err("an empty connection URL cannot name a database");

    assert!(
        error.to_string().contains("Database URL cannot be empty"),
        "got: {error}"
    );
}

#[tokio::test]
async fn an_unreachable_external_database_stops_the_flow() {
    let prompter = ScriptedPrompter::new([
        "coverage-tenant",
        "postgres://nobody:nobody@127.0.0.1:1/nothing",
    ]);

    let error = create_external_tenant(&prompter)
        .await
        .map(|_| ())
        .expect_err("a tenant must not be registered against a database nobody can reach");

    assert!(
        error.to_string().contains("Could not connect to database"),
        "got: {error}"
    );
}

#[tokio::test]
async fn validate_connection_is_false_for_a_port_nothing_listens_on() {
    assert!(
        !validate_connection("postgres://nobody:nobody@127.0.0.1:1/nothing").await,
        "a refused connection is not a validated one"
    );
}

#[tokio::test]
async fn validate_connection_is_false_for_a_url_that_is_not_a_connection_string() {
    assert!(
        !validate_connection("definitely not a url").await,
        "an unparseable URL must fail validation rather than panic"
    );
}
