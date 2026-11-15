//! `ContentHttpError` answers through the API error model: content and link
//! misses are 404, invalid requests and failed validations 400, a repository
//! failure keeps the repository classification, and every other content
//! failure is a 500 whose body carries the fixed message, never the cause.

use axum::body::to_bytes;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use systemprompt_api::kernel::{ApiHttpError, IntoApiError};
use systemprompt_content::ContentError;
use systemprompt_content_entry::http::ContentHttpError;
use systemprompt_models::api::ErrorCode;
use systemprompt_traits::RepositoryError;

fn code_of(err: ContentError) -> ErrorCode {
    ContentHttpError::from(err).into_api_error().code
}

async fn status_and_body(err: ContentHttpError) -> anyhow::Result<(StatusCode, String)> {
    let response = err.into_response();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await?;
    Ok((status, String::from_utf8(bytes.to_vec())?))
}

#[test]
fn content_errors_classify_by_variant() {
    assert_eq!(
        code_of(ContentError::ContentNotFound("a".into())),
        ErrorCode::NotFound
    );
    assert_eq!(
        code_of(ContentError::LinkNotFound("a".into())),
        ErrorCode::NotFound
    );
    assert_eq!(
        code_of(ContentError::InvalidRequest("a".into())),
        ErrorCode::BadRequest
    );
    assert_eq!(
        code_of(ContentError::Validation("a".into())),
        ErrorCode::BadRequest
    );
    assert_eq!(
        code_of(ContentError::Repository(RepositoryError::NotFound {
            entity: "content",
            key: Some("x".to_owned()),
        })),
        ErrorCode::NotFound
    );
    assert_eq!(
        code_of(ContentError::Parse("secret detail".into())),
        ErrorCode::InternalError
    );
}

#[tokio::test]
async fn a_content_failure_is_a_500_without_its_cause() -> anyhow::Result<()> {
    let (status, body) = status_and_body(ContentError::DatabaseNotPostgres.into()).await?;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(
        !body.contains("PostgreSQL"),
        "body leaked the cause: {body}"
    );
    let (status, body) =
        status_and_body(ContentError::Parse("secret detail".into()).into()).await?;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(
        !body.contains("secret detail"),
        "body leaked the cause: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn a_refusal_built_by_a_handler_passes_through_unchanged() -> anyhow::Result<()> {
    let (status, _) = status_and_body(ApiHttpError::not_found("Content not found").into()).await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = status_and_body(
        RepositoryError::Conflict {
            entity: "link",
            key: "x".to_owned(),
            reason: "taken".to_owned(),
        }
        .into(),
    )
    .await?;
    assert_eq!(status, StatusCode::CONFLICT);
    Ok(())
}
