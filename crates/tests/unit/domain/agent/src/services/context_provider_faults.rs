//! Failure mapping in the `ContextProvider` / `ContextMaterializer` adapter.
//!
//! The adapter is the seam other crates consume contexts through, so what it
//! turns a database outage into decides what those callers see. A dropped
//! connection must arrive as `Database`, never as `NotFound` — a caller that
//! reads "not found" for an outage will happily create a duplicate context or
//! report the user's history as empty.

use systemprompt_agent::repository::ContextRepository;
use systemprompt_agent::services::ContextProviderService;
use systemprompt_identifiers::{ContextId, SessionId};
use systemprompt_test_fixtures::{closed_db_pool, unique_user_id};
use systemprompt_traits::{
    ContextMaterializer, ContextProvider, ContextProviderError, EnsureContextParams,
};

async fn provider_on_a_dead_pool() -> ContextProviderService {
    let pool = closed_db_pool().await;
    ContextProviderService::new(ContextRepository::new(&pool).expect("context repo"))
}

fn assert_database(outcome: &Result<(), ContextProviderError>, verb: &str) {
    assert!(
        matches!(outcome, Err(ContextProviderError::Database(_))),
        "{verb} on a dead pool must report a database failure, got {outcome:?}"
    );
}

#[tokio::test]
async fn listing_contexts_against_a_dead_pool_reports_a_database_failure() {
    let provider = provider_on_a_dead_pool().await;

    let outcome = provider
        .list_contexts_with_stats(&unique_user_id("ctxfault"))
        .await
        .map(|_| ());

    assert_database(&outcome, "list_contexts_with_stats");
}

// Why: `get_context` has a NotFound arm. An outage must not fall into it —
// that would tell the caller the context is gone when it merely could not be
// read.
#[tokio::test]
async fn a_dead_pool_is_not_mistaken_for_a_missing_context() {
    let provider = provider_on_a_dead_pool().await;

    let outcome = provider
        .get_context(&ContextId::generate(), &unique_user_id("ctxfault"))
        .await
        .map(|_| ());

    assert_database(&outcome, "get_context");
}

#[tokio::test]
async fn creating_a_context_against_a_dead_pool_reports_a_database_failure() {
    let provider = provider_on_a_dead_pool().await;
    let session = SessionId::generate();

    let outcome = provider
        .create_context(&unique_user_id("ctxfault"), Some(&session), "ctx")
        .await
        .map(|_| ());

    assert_database(&outcome, "create_context");
}

#[tokio::test]
async fn renaming_a_context_against_a_dead_pool_is_not_reported_as_missing() {
    let provider = provider_on_a_dead_pool().await;

    let outcome = provider
        .update_context_name(
            &ContextId::generate(),
            &unique_user_id("ctxfault"),
            "new name",
        )
        .await;

    assert_database(&outcome, "update_context_name");
}

#[tokio::test]
async fn deleting_a_context_against_a_dead_pool_is_not_reported_as_missing() {
    let provider = provider_on_a_dead_pool().await;

    let outcome = provider
        .delete_context(&ContextId::generate(), &unique_user_id("ctxfault"))
        .await;

    assert_database(&outcome, "delete_context");
}

// Why: `kind` arrives as a free string from the caller. An unrecognised value
// must be rejected before it reaches the database, and as a caller error
// rather than a database one.
#[tokio::test]
async fn an_unrecognised_context_kind_is_rejected_before_any_query() {
    let provider = provider_on_a_dead_pool().await;
    let context_id = ContextId::generate();
    let user_id = unique_user_id("ctxfault");

    let outcome = provider
        .ensure_context(EnsureContextParams {
            context_id: &context_id,
            user_id: &user_id,
            session_id: None,
            name: "ctx",
            kind: "not-a-kind",
        })
        .await;

    assert!(
        matches!(outcome, Err(ContextProviderError::Internal(_))),
        "an unparseable kind is the caller's error, not the database's: {outcome:?}"
    );
}

#[tokio::test]
async fn a_valid_kind_reaches_the_database_and_surfaces_its_failure() {
    let provider = provider_on_a_dead_pool().await;
    let context_id = ContextId::generate();
    let user_id = unique_user_id("ctxfault");

    let outcome = provider
        .ensure_context(EnsureContextParams {
            context_id: &context_id,
            user_id: &user_id,
            session_id: None,
            name: "ctx",
            kind: "cli_session",
        })
        .await;

    assert_database(&outcome, "ensure_context");
}
