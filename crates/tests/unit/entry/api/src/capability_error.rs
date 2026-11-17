use axum::response::IntoResponse;
use http::StatusCode;
use serde_json::Value;
use systemprompt_extension::capability_error_response;
use systemprompt_extension::router::{ApiHttpError, IntoApiError};
use systemprompt_models::api::ApiError;

#[derive(Debug)]
enum WidgetError {
    Missing,
    Storage(std::io::Error),
    Http(ApiHttpError),
}

impl From<ApiHttpError> for WidgetError {
    fn from(error: ApiHttpError) -> Self {
        Self::Http(error)
    }
}

impl IntoApiError for WidgetError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::Missing => {
                ApiError::not_found("widget not found").with_error_key("widget_not_found")
            },
            Self::Storage(error) => ApiError::internal("widget storage failed", error),
            Self::Http(error) => error.into_api_error(),
        }
    }
}

capability_error_response!(WidgetError);

async fn answer(error: WidgetError) -> (StatusCode, Value) {
    let response = error.into_response();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("body");
    (status, serde_json::from_slice(&bytes).expect("json body"))
}

fn handler(fail: bool) -> Result<(), WidgetError> {
    let refused: Result<(), ApiHttpError> = if fail {
        Err(ApiHttpError::forbidden("not yours"))
    } else {
        Ok(())
    };
    refused?;
    Ok(())
}

#[tokio::test]
async fn a_client_error_keeps_its_status_key_and_message() {
    let (status, body) = answer(WidgetError::Missing).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error_key"], "widget_not_found");
    assert_eq!(body["message"], "widget not found");
}

#[tokio::test]
async fn a_server_error_serialises_the_fixed_message_never_the_cause() {
    let secret = "db password rejected for host 10.0.0.7";
    let (status, body) = answer(WidgetError::Storage(std::io::Error::other(secret))).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    let text = body.to_string();
    assert!(!text.contains(secret), "5xx body leaked its cause: {text}");
    assert!(
        !text.contains("widget storage failed"),
        "5xx body leaked its context: {text}"
    );
}

#[tokio::test]
async fn an_api_http_error_passes_through_unchanged_via_question_mark() {
    let error = handler(true).expect_err("refused");
    let (status, body) = answer(error).await;
    let (direct_status, direct_body) = {
        let response = ApiHttpError::forbidden("not yours").into_response();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("body");
        (
            status,
            serde_json::from_slice::<Value>(&bytes).expect("json"),
        )
    };
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(status, direct_status);
    assert_eq!(body["message"], direct_body["message"]);
    assert_eq!(body["code"], direct_body["code"]);
    assert!(handler(false).is_ok());
}
