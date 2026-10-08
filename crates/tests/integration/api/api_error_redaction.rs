//! The unified `ApiHttpError` mapping never puts an internal cause into a
//! response body: 5xx bodies carry the fixed public message, a rejected token
//! is a 401 without the verifier's text, and stored-data corruption is a 500.

use axum::body::to_bytes;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use systemprompt_api::error::ApiHttpError;
use systemprompt_content::ContentError;
use systemprompt_models::execution::ContextExtractionError;
use systemprompt_traits::RepositoryError;

const SECRET_CAUSE: &str = "pg: relation user_secrets leaked-detail";

async fn status_and_body(err: ApiHttpError) -> anyhow::Result<(StatusCode, String)> {
    let response = err.into_response();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await?;
    Ok((status, String::from_utf8(bytes.to_vec())?))
}

#[tokio::test]
async fn a_context_lookup_failure_is_a_500_without_its_cause() -> anyhow::Result<()> {
    let err = ContextExtractionError::DatabaseError {
        context: "user lookup".to_owned(),
        source: SECRET_CAUSE.into(),
    };
    let (status, body) = status_and_body(err.into()).await?;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(
        !body.contains("leaked-detail"),
        "body leaked the cause: {body}"
    );
    assert!(
        body.contains("Internal server error"),
        "unexpected body: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn stored_data_corruption_is_a_500_not_a_400() -> anyhow::Result<()> {
    let corrupt =
        RepositoryError::invalid_data("agent_name", format!("missing for task t-1 {SECRET_CAUSE}"));
    let (status, body) = status_and_body(corrupt.into()).await?;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(
        !body.contains("agent_name"),
        "body leaked the row detail: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn a_rejected_token_is_a_401_without_the_verifier_text() -> anyhow::Result<()> {
    let err = ContextExtractionError::InvalidToken(SECRET_CAUSE.into());
    let (status, body) = status_and_body(err.into()).await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(
        !body.contains("leaked-detail"),
        "body leaked the cause: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn content_errors_classify_by_variant() -> anyhow::Result<()> {
    let (missing, _) = status_and_body(ContentError::LinkNotFound("abc".to_owned()).into()).await?;
    assert_eq!(missing, StatusCode::NOT_FOUND);

    let (invalid, _) =
        status_and_body(ContentError::InvalidRequest("bad".to_owned()).into()).await?;
    assert_eq!(invalid, StatusCode::BAD_REQUEST);

    let (repo_missing, _) = status_and_body(
        ContentError::Repository(RepositoryError::not_found("content", "x")).into(),
    )
    .await?;
    assert_eq!(repo_missing, StatusCode::NOT_FOUND);

    let (failed, body) = status_and_body(ContentError::DatabaseNotPostgres.into()).await?;
    assert_eq!(failed, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(
        !body.contains("PostgreSQL"),
        "body leaked the cause: {body}"
    );
    Ok(())
}
