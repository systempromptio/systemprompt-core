use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::response::IntoResponse;
use systemprompt_api::routes::brain::{
    BrainHttpError, FieldsBody, ResolveBody, SearchParams, SimulateBody, admin_router, router,
};
use systemprompt_brain::BrainError;
use systemprompt_identifiers::{BrainFactId, FactKindId};
use systemprompt_traits::RepositoryError;
use tower::ServiceExt;

const FACT: &str = "m_0190f5c2-7c1e-7a3b-9f2a-3c4d5e6f7a8b";

async fn status_of(app: axum::Router, method: Method, path: &str) -> StatusCode {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    app.oneshot(request).await.unwrap().status()
}

#[tokio::test]
async fn disabled_user_routes_answer_404_on_every_path() {
    let paths = [
        (Method::GET, "/facts".to_owned()),
        (Method::POST, "/facts".to_owned()),
        (Method::GET, format!("/facts/{FACT}")),
        (Method::GET, format!("/facts/{FACT}/timeline")),
        (Method::GET, format!("/facts/{FACT}/why")),
        (Method::GET, "/subjects/person/alice".to_owned()),
        (Method::POST, format!("/facts/{FACT}/confirm")),
        (Method::POST, format!("/facts/{FACT}/dispute")),
        (Method::POST, format!("/facts/{FACT}/retract")),
        (Method::POST, format!("/facts/{FACT}/erase")),
        (Method::POST, format!("/facts/{FACT}/pin")),
        (Method::POST, format!("/facts/{FACT}/unpin")),
        (Method::PATCH, format!("/facts/{FACT}/fields")),
        (Method::POST, "/disputes/x/resolve".to_owned()),
        (Method::GET, "/proposals".to_owned()),
        (Method::POST, "/proposals/x/approve".to_owned()),
        (Method::POST, "/proposals/x/reject".to_owned()),
    ];
    for (method, path) in paths {
        let status = status_of(router(None), method.clone(), &path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{method} {path}");
    }
}

#[tokio::test]
async fn disabled_admin_simulate_answers_404() {
    let status = status_of(admin_router(None), Method::POST, "/simulate").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

fn mapped(error: BrainError) -> StatusCode {
    BrainHttpError::from(error).into_response().status()
}

#[test]
fn brain_errors_map_to_statuses() {
    let fact = BrainFactId::new(FACT);
    let kind = FactKindId::try_new("decision").unwrap();
    let cases = [
        (BrainError::NotFound(fact), StatusCode::NOT_FOUND),
        (
            BrainError::Forbidden {
                reason: "no".to_owned(),
            },
            StatusCode::FORBIDDEN,
        ),
        (BrainError::UnknownKind(kind), StatusCode::UNPROCESSABLE_ENTITY),
        (BrainError::Tombstoned, StatusCode::CONFLICT),
        (
            BrainError::Repository(RepositoryError::not_found("fact", FACT)),
            StatusCode::NOT_FOUND,
        ),
        (
            BrainError::Repository(RepositoryError::invalid_argument("limit", "bad")),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            BrainError::Repository(RepositoryError::database(std::io::Error::other("down"))),
            StatusCode::SERVICE_UNAVAILABLE,
        ),
        (
            BrainError::Database(sqlx::Error::PoolTimedOut),
            StatusCode::SERVICE_UNAVAILABLE,
        ),
    ];
    for (error, expected) in cases {
        let label = format!("{error:?}");
        assert_eq!(mapped(error), expected, "{label}");
    }
}

#[test]
fn unavailable_body_is_fixed_and_hides_the_cause() {
    let error = BrainError::Repository(RepositoryError::database(std::io::Error::other(
        "secret dsn",
    )));
    let api = BrainHttpError::from(error).into_inner();
    let body = serde_json::to_string(&api).unwrap();
    assert!(!body.contains("secret dsn"), "{body}");
}

#[test]
fn fields_body_rejects_unknown_top_level_keys() {
    let ok: Result<FieldsBody, _> = serde_json::from_str(r#"{"fields":{"title":"x"}}"#);
    assert!(ok.is_ok());
    let extra: Result<FieldsBody, _> =
        serde_json::from_str(r#"{"fields":{"title":"x"},"kind":"decision"}"#);
    assert!(extra.is_err());
}

#[test]
fn resolve_body_requires_a_known_outcome() {
    let body = format!(r#"{{"fact_id":"{FACT}","outcome":"upheld"}}"#);
    assert!(serde_json::from_str::<ResolveBody>(&body).is_ok());
    let bad = format!(r#"{{"fact_id":"{FACT}","outcome":"maybe"}}"#);
    assert!(serde_json::from_str::<ResolveBody>(&bad).is_err());
}

#[test]
fn search_params_reject_unsupported_filters() {
    let ok: Result<SearchParams, _> =
        serde_json::from_str(r#"{"kind":"decision","status":"confirmed,stale","limit":5}"#);
    assert!(ok.is_ok());
    let bad: Result<SearchParams, _> = serde_json::from_str(r#"{"cursor":"abc"}"#);
    assert!(bad.is_err());
}

#[test]
fn simulate_body_parses_client_and_defaults_reuse() {
    let body: SimulateBody =
        serde_json::from_str(r#"{"user_id":"u1","client":"claude-code","prompt":"hi"}"#).unwrap();
    assert!(!body.reuse);
    assert_eq!(body.prompt, "hi");
}
