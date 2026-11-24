use anyhow::Result;
use axum::body::Body;
use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use systemprompt_test_fixtures::seed_bridge_credential;
use uuid::Uuid;

use super::documents::{app, create_markdown, fixture, id_of, json_request, request, send};

fn level_of<'a>(found: &'a Value, user: &str) -> Option<&'a Value> {
    found["subjects"]
        .as_array()
        .and_then(|subjects| subjects.iter().find(|s| s["value"] == user))
        .map(|s| &s["access_level"])
}

#[tokio::test]
async fn subject_search_on_a_document_reports_each_users_level() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let base = format!("lvl{}", Uuid::new_v4().simple());
    let owner = seed_bridge_credential(&pool, &format!("{base}-owner")).await?;
    let reader = seed_bridge_credential(&pool, &format!("{base}-reader")).await?;
    let stranger = seed_bridge_credential(&pool, &format!("{base}-stranger")).await?;
    let id = id_of(&create_markdown(&state, &owner).await?);
    let shared = send(
        app(&state, &owner),
        json_request(
            Method::POST,
            &format!("/{id}/grants"),
            &json!({
                "subject_kind": "user",
                "subject_value": reader.user_id.as_str(),
                "level": "view"
            }),
        ),
    )
    .await?;
    assert_eq!(shared.status, StatusCode::CREATED, "{}", shared.json());

    let found = send(
        app(&state, &owner),
        request(
            Method::GET,
            &format!("/subjects?kind=user&q={base}&document_id={id}"),
        )
        .body(Body::empty())
        .expect("request"),
    )
    .await?;
    assert_eq!(found.status, StatusCode::OK, "{}", found.json());
    let body = found.json();
    assert_eq!(body["kind"], "user");
    assert_eq!(
        level_of(&body, reader.user_id.as_str()),
        Some(&json!("view")),
        "{body}"
    );
    assert_eq!(
        level_of(&body, stranger.user_id.as_str()),
        Some(&json!("none")),
        "{body}"
    );
    let own = level_of(&body, owner.user_id.as_str()).expect("owner listed");
    assert!(own.is_string() && own != "none", "{body}");
    Ok(())
}

#[tokio::test]
async fn subject_search_without_a_document_carries_no_levels() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let base = format!("nolvl{}", Uuid::new_v4().simple());
    let owner = seed_bridge_credential(&pool, &format!("{base}-owner")).await?;
    let found = send(
        app(&state, &owner),
        request(Method::GET, &format!("/subjects?kind=user&q={base}"))
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    assert_eq!(found.status, StatusCode::OK);
    let body = found.json();
    let subject = body["subjects"]
        .as_array()
        .and_then(|s| s.iter().find(|s| s["value"] == owner.user_id.as_str()))
        .expect("owner found");
    assert!(subject.get("access_level").is_none(), "{body}");
    Ok(())
}

#[tokio::test]
async fn a_role_search_ignores_the_document_levels() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "lvl-role-owner").await?;
    let id = id_of(&create_markdown(&state, &owner).await?);
    let found = send(
        app(&state, &owner),
        request(
            Method::GET,
            &format!("/subjects?kind=role&q=us&document_id={id}"),
        )
        .body(Body::empty())
        .expect("request"),
    )
    .await?;
    assert_eq!(found.status, StatusCode::OK);
    let body = found.json();
    let roles = body["subjects"].as_array().expect("roles");
    assert!(!roles.is_empty(), "{body}");
    assert!(
        roles.iter().all(|r| r.get("access_level").is_none()),
        "{body}"
    );
    Ok(())
}
