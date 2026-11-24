use anyhow::Result;
use axum::body::Body;
use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use systemprompt_files_entry::http::documents::DocumentsState;
use systemprompt_test_fixtures::{AuthedFixture, seed_bridge_credential};
use uuid::Uuid;

use super::documents::{Reply, app, create_markdown, fixture, id_of, json_request, request, send};

async fn get(state: &DocumentsState, user: &AuthedFixture, uri: &str) -> Result<Reply> {
    send(
        app(state, user),
        request(Method::GET, uri)
            .body(Body::empty())
            .expect("request"),
    )
    .await
}

fn person<'a>(body: &'a Value, user: &AuthedFixture) -> Option<&'a Value> {
    body["people"].as_array().and_then(|people| {
        people
            .iter()
            .find(|p| p["user_id"] == user.user_id.as_str())
    })
}

fn assert_refused(reply: &Reply) {
    assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{}", reply.json());
    assert_eq!(
        reply.json()["error_key"],
        "invalid_params",
        "{}",
        reply.json()
    );
}

async fn share_view(
    state: &DocumentsState,
    owner: &AuthedFixture,
    id: &str,
    to: &AuthedFixture,
) -> Result<()> {
    let shared = send(
        app(state, owner),
        json_request(
            Method::POST,
            &format!("/{id}/grants"),
            &json!({
                "subject_kind": "user",
                "subject_value": to.user_id.as_str(),
                "level": "view"
            }),
        ),
    )
    .await?;
    assert_eq!(shared.status, StatusCode::CREATED, "{}", shared.json());
    Ok(())
}

#[tokio::test]
async fn people_search_answers_the_tool_fields() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let base = format!("ppl{}", Uuid::new_v4().simple());
    let owner = seed_bridge_credential(&pool, &format!("{base}-owner")).await?;
    let reader = seed_bridge_credential(&pool, &format!("{base}-reader")).await?;

    let found = get(&state, &owner, &format!("/people?query={base}")).await?;
    assert_eq!(found.status, StatusCode::OK, "{}", found.json());
    let body = found.json();
    let entry = person(&body, &reader).expect("reader found");
    assert_eq!(entry["email"], reader.email.as_str(), "{body}");
    assert!(
        entry["name"].as_str().is_some_and(|n| !n.is_empty()),
        "{body}"
    );
    assert!(entry.get("access").is_none(), "{body}");
    let keys: Vec<&String> = entry.as_object().expect("object").keys().collect();
    assert_eq!(keys.len(), 3, "{body}");

    let id = id_of(&create_markdown(&state, &owner).await?);
    share_view(&state, &owner, &id, &reader).await?;
    let found = get(
        &state,
        &owner,
        &format!("/people?query={base}&limit=5&document_id={id}"),
    )
    .await?;
    assert_eq!(found.status, StatusCode::OK, "{}", found.json());
    let body = found.json();
    assert_eq!(
        person(&body, &reader).expect("reader")["access"],
        "view",
        "{body}"
    );
    Ok(())
}

#[tokio::test]
async fn people_by_ids_leaves_out_unknown_ids() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let base = format!("pplid{}", Uuid::new_v4().simple());
    let owner = seed_bridge_credential(&pool, &format!("{base}-owner")).await?;
    let reader = seed_bridge_credential(&pool, &format!("{base}-reader")).await?;
    let stranger = seed_bridge_credential(&pool, &format!("{base}-stranger")).await?;
    let id = id_of(&create_markdown(&state, &owner).await?);
    share_view(&state, &owner, &id, &reader).await?;

    let ids = format!(
        "{},user-{}-missing,{}",
        reader.user_id.as_str(),
        Uuid::new_v4().simple(),
        stranger.user_id.as_str()
    );
    let found = get(&state, &owner, &format!("/people?ids={ids}")).await?;
    assert_eq!(found.status, StatusCode::OK, "{}", found.json());
    let body = found.json();
    let people = body["people"].as_array().expect("people");
    assert_eq!(people.len(), 2, "{body}");
    assert_eq!(people[0]["user_id"], reader.user_id.as_str(), "{body}");
    assert_eq!(people[0]["email"], reader.email.as_str(), "{body}");
    assert_eq!(people[1]["user_id"], stranger.user_id.as_str(), "{body}");
    assert!(people.iter().all(|p| p.get("access").is_none()), "{body}");

    let found = get(
        &state,
        &owner,
        &format!("/people?ids={ids}&document_id={id}"),
    )
    .await?;
    assert_eq!(found.status, StatusCode::OK, "{}", found.json());
    let body = found.json();
    assert_eq!(
        person(&body, &reader).expect("reader")["access"],
        "view",
        "{body}"
    );
    assert!(
        person(&body, &stranger)
            .expect("stranger")
            .get("access")
            .is_none(),
        "{body}"
    );
    Ok(())
}

#[tokio::test]
async fn people_refuses_a_bad_request_with_invalid_params() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "ppl-refused").await?;
    assert_refused(&get(&state, &owner, "/people?query=%20").await?);
    assert_refused(&get(&state, &owner, "/people?query=a&ids=b").await?);
    assert_refused(&get(&state, &owner, "/people?ids=,").await?);
    let many: Vec<String> = (0..21).map(|n| format!("user-{n}")).collect();
    assert_refused(&get(&state, &owner, &format!("/people?ids={}", many.join(","))).await?);
    let twenty: Vec<String> = (0..20).map(|n| format!("user-{n}")).collect();
    let found = get(&state, &owner, &format!("/people?ids={}", twenty.join(","))).await?;
    assert_eq!(found.status, StatusCode::OK, "{}", found.json());
    Ok(())
}

#[tokio::test]
async fn the_people_segment_is_not_a_document_id() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "ppl-route").await?;
    let bare = get(&state, &owner, "/people").await?;
    assert_refused(&bare);
    assert_eq!(
        bare.json()["message"],
        "give exactly one of query or ids",
        "{}",
        bare.json()
    );
    let found = get(&state, &owner, "/people?query=ppl-route").await?;
    assert_eq!(found.status, StatusCode::OK, "{}", found.json());
    assert!(person(&found.json(), &owner).is_some(), "{}", found.json());
    Ok(())
}

#[tokio::test]
async fn a_rest_comment_accepts_mention_options() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let base = format!("pplmo{}", Uuid::new_v4().simple());
    let owner = seed_bridge_credential(&pool, &format!("{base}-owner")).await?;
    let reader = seed_bridge_credential(&pool, &format!("{base}-reader")).await?;
    let id = id_of(&create_markdown(&state, &owner).await?);
    share_view(&state, &owner, &id, &reader).await?;
    let outline = get(&state, &owner, &format!("/{id}/read?view=outline"))
        .await?
        .json();
    let line = outline["lines"]
        .as_array()
        .and_then(|lines| lines.first())
        .and_then(Value::as_str)
        .expect("a line");
    let (anchor, _) = line.split_once('|').expect("hashline");
    let node = anchor.rsplit_once(':').expect("node:hash").0.to_owned();
    let reader_id = reader.user_id.as_str();
    let comment = send(
        app(&state, &owner),
        json_request(
            Method::POST,
            &format!("/{id}/comments"),
            &json!({
                "anchor": { "node": node },
                "body": format!("look @[Reader](user:{reader_id})"),
                "mentions": [reader_id],
                "mention_options": [{ "user_id": reader_id, "attention": "human" }]
            }),
        ),
    )
    .await?;
    assert_eq!(comment.status, StatusCode::CREATED, "{}", comment.json());
    assert_eq!(
        comment.json()["mentions"],
        json!([reader_id]),
        "{}",
        comment.json()
    );

    let refused = send(
        app(&state, &owner),
        json_request(
            Method::POST,
            &format!("/{id}/comments"),
            &json!({
                "anchor": { "node": node },
                "body": "nobody mentioned",
                "mention_options": [{ "user_id": reader_id, "attention": "human" }]
            }),
        ),
    )
    .await?;
    assert_refused(&refused);
    Ok(())
}
