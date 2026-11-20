//! Where a person's UI session is accepted: only the documents, folders and
//! uploads routes take it, and act as the person; MCP, the gateway, A2A, the
//! bridge stream and the other session routes refuse it by its audience; sent
//! as the session cookie it is refused; and a high-impact action wants a
//! sign-in no older than the step-up window (RFC 9470). The gateway is off in
//! this fixture; its decode is the first-party session policy, whose refusal of
//! the `ui` audience the security unit tests pin.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use serde_json::{Value, json};

use systemprompt_manifest::profile::UiSessionConfig;

use super::routes_oauth_person_ui::{Person, claims, person, person_with, send, sign_in};

fn bearer(
    method: Method,
    uri: &str,
    token: &str,
    body: Option<&Value>,
) -> anyhow::Result<Request<Body>> {
    let builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, "127.0.0.1")
        .header(header::AUTHORIZATION, format!("Bearer {token}"));
    Ok(match body {
        Some(body) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))?,
        None => builder.body(Body::empty())?,
    })
}

async fn ui_token(p: &Person, signed_in_ago_secs: i64) -> anyhow::Result<String> {
    let (status, v) = sign_in(p, signed_in_ago_secs).await?;
    assert_eq!(status, StatusCode::OK, "{v}");
    let token = v["access_token"].as_str().unwrap_or_default().to_owned();
    assert_eq!(claims(&token)["ui_session"], Value::Bool(true));
    Ok(token)
}

async fn create_document(p: &Person, token: &str) -> anyhow::Result<Value> {
    let body = json!({
        "template_id": "blank-markdown",
        "name": format!("ui-{}.md", uuid::Uuid::new_v4().simple()),
        "params": { "values": { "title": "Plan" } }
    });
    let (status, text) = send(
        &p.app,
        bearer(Method::POST, "/api/v1/documents", token, Some(&body))?,
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{text}");
    Ok(serde_json::from_str(
        text.split_once('\n').map_or("", |(_, b)| b),
    )?)
}

#[tokio::test]
async fn the_documents_routes_take_a_ui_session_as_the_person() -> anyhow::Result<()> {
    let p = person(true).await?;
    let token = ui_token(&p, 5).await?;
    let created = create_document(&p, &token).await?;
    assert_eq!(created["revision"]["actor_kind"], "user", "{created}");
    for uri in ["/api/v1/documents", "/api/v1/document-folders"] {
        let (status, text) = send(&p.app, bearer(Method::GET, uri, &token, None)?).await?;
        assert_eq!(status, StatusCode::OK, "{uri}: {text}");
    }
    Ok(())
}

#[tokio::test]
async fn every_other_surface_refuses_a_ui_session() -> anyhow::Result<()> {
    let p = person(true).await?;
    let token = ui_token(&p, 5).await?;
    for (method, uri) in [
        (Method::GET, "/api/v1/core/contexts"),
        (Method::POST, "/api/v1/mcp/documents/mcp"),
        (Method::POST, "/api/v1/agents/assistant"),
        (Method::GET, "/v1/bridge/stream"),
        (Method::GET, "/v1/bridge/whoami"),
    ] {
        let (status, text) = send(&p.app, bearer(method, uri, &token, Some(&json!({})))?).await?;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}: {text}");
    }
    let first_party = person(false).await?;
    let (_, v) = sign_in(&first_party, 5).await?;
    let first = v["access_token"].as_str().unwrap_or_default();
    let (status, text) = send(
        &first_party.app,
        bearer(Method::GET, "/api/v1/core/contexts", first, None)?,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{text}");
    Ok(())
}

#[tokio::test]
async fn a_ui_session_sent_as_the_cookie_is_refused() -> anyhow::Result<()> {
    let p = person(true).await?;
    let token = ui_token(&p, 5).await?;
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/documents")
        .header(header::HOST, "127.0.0.1")
        .header(header::COOKIE, format!("access_token={token}"))
        .header("sec-fetch-site", "same-origin")
        .body(Body::empty())?;
    let (status, text) = send(&p.app, req).await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{text}");
    Ok(())
}

#[tokio::test]
async fn high_impact_actions_want_a_recent_sign_in() -> anyhow::Result<()> {
    let p = person(true).await?;
    let fresh = ui_token(&p, 5).await?;
    let window = p.ui_session.step_up_max_age_secs;
    assert_eq!(window, 600);
    let stale = ui_token(&p, 2 * window).await?;
    let id = create_document(&p, &fresh).await?["document"]["id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let share = json!({ "subject_kind": "user", "subject_id": p.user.as_str(), "level": "view" });
    let cap = json!({ "cap": "comment" });
    for (method, uri, body) in [
        (
            Method::POST,
            format!("/api/v1/documents/{id}/grants"),
            Some(&share),
        ),
        (
            Method::DELETE,
            format!("/api/v1/documents/{id}/grants"),
            Some(&share),
        ),
        (
            Method::DELETE,
            format!("/api/v1/documents/{id}/grants/all"),
            None,
        ),
        (
            Method::POST,
            format!("/api/v1/documents/{id}/transfer"),
            Some(&share),
        ),
        (
            Method::PUT,
            format!("/api/v1/documents/{id}/agent-cap"),
            Some(&cap),
        ),
        (Method::DELETE, format!("/api/v1/documents/{id}"), None),
        (
            Method::DELETE,
            format!("/api/v1/document-folders/{id}"),
            None,
        ),
    ] {
        let (status, text) = send(&p.app, bearer(method, &uri, &stale, body)?).await?;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}: {text}");
        assert!(
            text.contains("error=\"insufficient_user_authentication\""),
            "{uri}: {text}"
        );
        assert!(text.contains(&format!("max_age={window}")), "{uri}: {text}");
    }
    let (status, text) = send(
        &p.app,
        bearer(
            Method::GET,
            &format!("/api/v1/documents/{id}"),
            &stale,
            None,
        )?,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{text}");
    let (status, text) = send(
        &p.app,
        bearer(
            Method::DELETE,
            &format!("/api/v1/documents/{id}"),
            &fresh,
            None,
        )?,
    )
    .await?;
    assert_ne!(status, StatusCode::UNAUTHORIZED, "{text}");
    Ok(())
}

#[tokio::test]
async fn the_configured_step_up_window_decides_and_is_reported() -> anyhow::Result<()> {
    let signed_in_ago = 300;
    let grant_uri = |id: &str| format!("/api/v1/documents/{id}/grants/all");

    let short = person_with(
        true,
        UiSessionConfig {
            step_up_max_age_secs: 120,
            ..UiSessionConfig::default()
        },
    )
    .await?;
    let fresh = ui_token(&short, 5).await?;
    let id = create_document(&short, &fresh).await?["document"]["id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let token = ui_token(&short, signed_in_ago).await?;
    let (status, text) = send(
        &short.app,
        bearer(Method::DELETE, &grant_uri(&id), &token, None)?,
    )
    .await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{text}");
    assert!(
        text.contains("error=\"insufficient_user_authentication\""),
        "{text}"
    );
    assert!(text.contains("max_age=120"), "{text}");
    assert!(!text.contains("max_age=600"), "{text}");

    let default = person(true).await?;
    let fresh = ui_token(&default, 5).await?;
    let id = create_document(&default, &fresh).await?["document"]["id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let token = ui_token(&default, signed_in_ago).await?;
    let (status, text) = send(
        &default.app,
        bearer(Method::DELETE, &grant_uri(&id), &token, None)?,
    )
    .await?;
    assert_ne!(status, StatusCode::UNAUTHORIZED, "{text}");
    Ok(())
}
