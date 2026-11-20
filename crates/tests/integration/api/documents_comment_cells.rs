use anyhow::Result;
use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use axum::{Extension, Router};
use serde_json::{Value, json};
use systemprompt_api::routes::documents::{DocumentsState, router};
use systemprompt_test_fixtures::{AuthedFixture, seed_bridge_credential};
use tower::ServiceExt;
use uuid::Uuid;

use super::common::{browser_page, request_context, setup_ctx};

async fn send(app: Router, req: Request<Body>) -> Result<(StatusCode, Value)> {
    let resp = app.oneshot(req).await?;
    let status = resp.status();
    let body = to_bytes(resp.into_body(), 16 * 1024 * 1024).await?;
    Ok((status, serde_json::from_slice(&body).unwrap_or(Value::Null)))
}

fn app(state: &DocumentsState, user: &AuthedFixture) -> Router {
    browser_page(router(state.clone())).layer(Extension(
        request_context(user.user_id.as_str()).with_browser_session(),
    ))
}

fn json_request(method: Method, uri: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .expect("request")
}

async fn created(
    state: &DocumentsState,
    owner: &AuthedFixture,
    template: &str,
    name: &str,
) -> Result<String> {
    let body = json!({
        "template_id": template,
        "name": format!("{name}-{}", Uuid::new_v4().simple()),
        "params": { "values": { "title": "Plan" } }
    });
    let (status, created) = send(app(state, owner), json_request(Method::POST, "/", &body)).await?;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    Ok(created["document"]["id"].as_str().expect("id").to_owned())
}

async fn first_node(state: &DocumentsState, owner: &AuthedFixture, id: &str) -> Result<String> {
    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/{id}/read?view=outline"))
        .body(Body::empty())?;
    let (_, outline) = send(app(state, owner), req).await?;
    let line = outline["lines"]
        .as_array()
        .and_then(|lines| lines.first())
        .and_then(Value::as_str)
        .expect("a line")
        .to_owned();
    let (anchor, _) = line.split_once('|').expect("hashline");
    Ok(anchor.rsplit_once(':').expect("node:hash").0.to_owned())
}

async fn comment(
    state: &DocumentsState,
    owner: &AuthedFixture,
    id: &str,
    anchor: Value,
) -> Result<(StatusCode, Value)> {
    send(
        app(state, owner),
        json_request(
            Method::POST,
            &format!("/{id}/comments"),
            &json!({ "anchor": anchor, "body": "check these" }),
        ),
    )
    .await
}

#[tokio::test]
async fn a_cell_range_comment_round_trips_over_rest() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let state = DocumentsState::build(&ctx)?;
    let owner = seed_bridge_credential(&pool, "docs-cells").await?;
    let id = created(&state, &owner, "blank-xlsx", "figures.xlsx").await?;
    let sheet = first_node(&state, &owner, &id).await?;
    let (status, made) = comment(
        &state,
        &owner,
        &id,
        json!({ "node": sheet, "cells": "b2:d9" }),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{made}");
    assert_eq!(made["anchor"]["cells"], "B2:D9");
    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/{id}/comments"))
        .body(Body::empty())?;
    let (status, listed) = send(app(&state, &owner), req).await?;
    assert_eq!(status, StatusCode::OK, "{listed}");
    assert_eq!(listed["comments"][0]["anchor"]["cells"], "B2:D9");

    let (status, refused) = comment(
        &state,
        &owner,
        &id,
        json!({ "node": sheet, "cells": "B2", "find": "x" }),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
    let (status, malformed) = comment(
        &state,
        &owner,
        &id,
        json!({ "node": sheet, "cells": "D9:B2" }),
    )
    .await?;
    assert!(status.is_client_error(), "{status} {malformed}");
    Ok(())
}

#[tokio::test]
async fn a_cell_range_on_markdown_is_a_bad_request() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let state = DocumentsState::build(&ctx)?;
    let owner = seed_bridge_credential(&pool, "docs-cells-md").await?;
    let id = created(&state, &owner, "blank-markdown", "notes.md").await?;
    let node = first_node(&state, &owner, &id).await?;
    let (status, refused) =
        comment(&state, &owner, &id, json!({ "node": node, "cells": "B2" })).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
    Ok(())
}
