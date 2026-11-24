//! Content delivery handlers: the not-found and repository-failure arms.
//!
//! Every one of these three handlers answers a repository failure with a 500
//! carrying the error string, and a miss with a 404. A closed pool drives the
//! failure arm without needing a broken row, and an unseeded slug drives the
//! miss.

use axum::Extension;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use systemprompt_content_entry::http::{
    get_content_handler, get_content_markdown_handler, list_content_by_source_handler,
};
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::AppContextSeeds as _;
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_app_context};

use crate::common::{content_state, negotiating_state};

async fn dead_context() -> std::sync::Arc<AppContext> {
    let boot = ensure_test_bootstrap();
    let ctx = test_app_context(&boot.database_url).await;
    ctx.db_pool().pool().close().await;
    ctx.db_pool().write_pool().close().await;
    ctx
}

fn req_ctx() -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::try_new("content").expect("valid AgentName"),
        Actor::user(UserId::new("00000000-0000-4000-8000-000000000001")),
    )
}

#[tokio::test]
async fn listing_a_source_reports_a_repository_failure_as_a_server_error() {
    let ctx = dead_context().await;

    let response =
        list_content_by_source_handler(State(content_state(&ctx)), Path("blog".to_owned()))
            .await
            .into_response();

    assert_eq!(
        response.status(),
        StatusCode::INTERNAL_SERVER_ERROR,
        "an unreachable database is the server's failure, not the caller's"
    );
}

#[tokio::test]
async fn fetching_one_document_reports_a_repository_failure_as_a_server_error() {
    let ctx = dead_context().await;

    let response = get_content_handler(
        State(content_state(&ctx)),
        Extension(req_ctx()),
        None,
        Path(("blog".to_owned(), "anything".to_owned())),
    )
    .await
    .into_response();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn fetching_markdown_reports_a_repository_failure_as_a_server_error() {
    let ctx = dead_context().await;

    let response = get_content_markdown_handler(
        State(negotiating_state(&ctx)),
        Extension(req_ctx()),
        Path(("blog".to_owned(), "anything".to_owned(), "md".to_owned())),
    )
    .await
    .into_response();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn a_slug_that_does_not_exist_is_a_not_found_rather_than_an_empty_document() {
    let boot = ensure_test_bootstrap();
    let ctx = test_app_context(&boot.database_url).await;

    let slug = format!("no-such-slug-{}", uuid::Uuid::new_v4().simple());
    let json = get_content_handler(
        State(content_state(&ctx)),
        Extension(req_ctx()),
        None,
        Path(("blog".to_owned(), slug.clone())),
    )
    .await
    .into_response();
    assert_eq!(json.status(), StatusCode::NOT_FOUND);

    let markdown = get_content_markdown_handler(
        State(negotiating_state(&ctx)),
        Extension(req_ctx()),
        Path(("blog".to_owned(), slug.clone(), "md".to_owned())),
    )
    .await
    .into_response();
    assert_eq!(markdown.status(), StatusCode::NOT_FOUND);
}
